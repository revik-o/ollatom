use super::super::{
    AtomicCommitHandle, AtomicCommitIdentifier, AtomicCommitStatus, AtomicMetadataPolicy,
    PreparedAtomicWrite, WorkspaceFile, io_error,
};

use crate::FilesystemError;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;

#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;

#[cfg(windows)]
use cap_std::fs::OpenOptionsExt;
use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;

static NEXT_ATOMIC_COMMIT_IDENTIFIER: AtomicU64 = AtomicU64::new(1);

impl WorkspaceFile {
    pub async fn prepare_atomic_write(
        &self,
        contents: Vec<u8>,
        metadata_policy: AtomicMetadataPolicy,
    ) -> Result<PreparedAtomicWrite, FilesystemError> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || {
            this.prepare_atomic_write_blocking(contents, metadata_policy)
        })
        .await
        .map_err(|source| {
            FilesystemError::throw_filesystem_operation_task_failed(
                self.relative_path.0.clone(),
                source,
            )
        })?
    }
    fn prepare_atomic_write_blocking(
        &self,
        contents: Vec<u8>,
        metadata_policy: AtomicMetadataPolicy,
    ) -> Result<PreparedAtomicWrite, FilesystemError> {
        if metadata_policy == AtomicMetadataPolicy::RequirePreserved {
            return Err(FilesystemError::AtomicMetadataPolicyUnsupported {
                path: self.relative_path.0.clone(),
            });
        }

        let (parent, destination_name) = self.open_parent()?;

        if let Ok(metadata) = parent.symlink_metadata(&destination_name)
            && metadata.file_type().is_symlink()
        {
            return Err(FilesystemError::SymbolicLinkNotAllowed {
                path: self.relative_path.0.clone(),
            });
        }

        let identifier = next_atomic_commit_identifier()?.0;
        let temporary_name = OsString::from(format!(".ollatom-atomic-{identifier}.tmp"));
        let mut options = OpenOptions::new();

        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);

        #[cfg(unix)]
        options.mode(0o600);

        #[cfg(windows)]
        options
            .access_mode(
                crate::permissions::windows_file_access::FILE_GENERIC_WRITE
                    | crate::permissions::windows_file_access::FILE_READ_ATTRIBUTES
                    | crate::permissions::windows_file_access::READ_CONTROL
                    | crate::permissions::windows_file_access::WRITE_DAC,
            )
            .share_mode(crate::permissions::windows_file_access::FILE_SHARE_NONE);

        let mut temporary_file = parent
            .open_with(&temporary_name, &options)
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        crate::permissions::restrict_capability_file_to_current_user(&mut temporary_file)
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        temporary_file
            .write_all(&contents)
            .and_then(|()| temporary_file.sync_all())
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        if metadata_policy == AtomicMetadataPolicy::PreserveSupported
            && let Ok(metadata) = parent.metadata(&destination_name)
        {
            temporary_file
                .set_permissions(metadata.permissions())
                .and_then(|()| temporary_file.sync_all())
                .map_err(|source| io_error(&self.relative_path.0, source))?;
        }

        Ok(PreparedAtomicWrite {
            parent_directory: Arc::new(parent),
            destination_name,
            temporary_name,
            destination_path: self.relative_path.0.clone(),
        })
    }
}

impl PreparedAtomicWrite {
    pub fn start_commit(mut self) -> Result<AtomicCommitHandle, FilesystemError> {
        let operation_identifier = next_atomic_commit_identifier()?;
        let (completion_sender, completion_receiver) = watch::channel(AtomicCommitStatus::Pending);
        let parent_directory = self.parent_directory.clone();
        let destination_name = self.destination_name.clone();
        let temporary_name = self.temporary_name.clone();
        let destination_path = self.destination_path.clone();

        self.temporary_name.clear();

        tokio::task::spawn_blocking(move || {
            let status = match parent_directory.rename(
                &temporary_name,
                &parent_directory,
                &destination_name,
            ) {
                Ok(()) => match parent_directory.open(".").and_then(|file| file.sync_all()) {
                    Ok(()) => AtomicCommitStatus::Succeeded,
                    Err(source) => AtomicCommitStatus::Failed(source.to_string()),
                },
                Err(source) => {
                    AtomicCommitStatus::Failed(io_error(&destination_path, source).to_string())
                }
            };

            completion_sender.send_replace(status);
        });

        Ok(AtomicCommitHandle {
            operation_identifier,
            completion_receiver,
        })
    }
}

impl Drop for PreparedAtomicWrite {
    fn drop(&mut self) {
        if !self.temporary_name.is_empty() {
            let _ = self.parent_directory.remove_file(&self.temporary_name);
        }
    }
}

impl AtomicCommitHandle {
    pub fn operation_identifier(&self) -> AtomicCommitIdentifier {
        self.operation_identifier
    }

    pub fn status(&self) -> AtomicCommitStatus {
        self.completion_receiver.borrow().clone()
    }

    pub async fn wait(mut self) -> AtomicCommitStatus {
        while *self.completion_receiver.borrow() == AtomicCommitStatus::Pending {
            if self.completion_receiver.changed().await.is_err() {
                break;
            }
        }

        self.completion_receiver.borrow().clone()
    }
}

fn next_atomic_commit_identifier() -> Result<AtomicCommitIdentifier, FilesystemError> {
    NEXT_ATOMIC_COMMIT_IDENTIFIER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(AtomicCommitIdentifier)
        .map_err(|_| FilesystemError::AtomicCommitIdentifierExhausted)
}
