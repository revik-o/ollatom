use crate::FilesystemError;
use cap_fs_ext::MetadataExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use std::ffi::OsString;
use std::fs::canonicalize;
use std::io::Error;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::watch;

#[cfg(windows)]
use cap_std::fs::Metadata;
#[cfg(windows)]
use std::io::ErrorKind;

mod file;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectoryIdentity {
    #[cfg(unix)]
    Unix {
        device_identifier: u64,
        inode_identifier: u64,
    },

    #[cfg(windows)]
    Windows {
        volume_identifier: Vec<u8>,
        file_identifier: [u8; 16],
    },
}

#[derive(Clone, Debug)]
pub struct WorkspaceDirectory {
    directory: Arc<Dir>,
    canonical_directory_path: Arc<PathBuf>,
    filesystem_identity: DirectoryIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRelativePath(PathBuf);

#[derive(Clone, Debug)]
pub struct WorkspaceFile {
    workspace_directory: WorkspaceDirectory,
    relative_path: WorkspaceRelativePath,
}

#[derive(Clone)]
pub enum WorkspaceCapability {
    Directory(WorkspaceDirectory),
    File(WorkspaceFile),
    Rename {
        source: WorkspaceFile,
        destination: WorkspaceFile,
    },
}

impl std::fmt::Debug for WorkspaceCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Directory(_) => "WorkspaceCapability::Directory(..)",
            Self::File(_) => "WorkspaceCapability::File(..)",
            Self::Rename { .. } => "WorkspaceCapability::Rename { .. }",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicMetadataPolicy {
    ContentOnly,
    PreserveSupported,
    RequirePreserved,
}

pub struct PreparedAtomicWrite {
    parent_directory: Arc<Dir>,
    destination_name: OsString,
    temporary_name: OsString,
    destination_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AtomicCommitIdentifier(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtomicCommitStatus {
    Pending,
    Succeeded,
    Failed(String),
}

pub struct AtomicCommitHandle {
    operation_identifier: AtomicCommitIdentifier,
    completion_receiver: watch::Receiver<AtomicCommitStatus>,
}

impl WorkspaceDirectory {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, FilesystemError> {
        let requested_path = path.as_ref().to_owned();

        tokio::task::spawn_blocking(move || {
            let canonical_directory_path =
                canonicalize(&requested_path).map_err(|source| io_error(&requested_path, source))?;

            let directory = Dir::open_ambient_dir(&canonical_directory_path, ambient_authority())
                .map_err(|source| io_error(&canonical_directory_path, source))?;

            let dir_metadata = directory
                .dir_metadata()
                .map_err(|source| io_error(&canonical_directory_path, source))?;

            if !dir_metadata.is_dir() {
                return Err(FilesystemError::NotRegularFile {
                    path: canonical_directory_path,
                });
            }

            #[cfg(unix)]
            let filesystem_identity = {
                DirectoryIdentity::Unix {
                    device_identifier: dir_metadata.dev(),
                    inode_identifier: dir_metadata.ino(),
                }
            };

            #[cfg(windows)]
            let filesystem_identity = directory_filesystem_identity(&canonical_directory_path, &dir_metadata)?;

            Ok(Self {
                directory: Arc::new(directory),
                canonical_directory_path: Arc::new(canonical_directory_path),
                filesystem_identity,
            })
        })
        .await
        .map_err(|source| FilesystemError::throw_filesystem_operation_task_failed(path.as_ref(), source))?
    }

    pub fn canonical_directory_path(&self) -> &Path {
        self.canonical_directory_path.as_path()
    }

    pub fn filesystem_identity(&self) -> &DirectoryIdentity {
        &self.filesystem_identity
    }

    pub fn file(&self, path: impl AsRef<Path>) -> Result<WorkspaceFile, FilesystemError> {
        Ok(WorkspaceFile {
            workspace_directory: self.clone(),
            relative_path: WorkspaceRelativePath::new(path)?,
        })
    }
}

#[cfg(windows)]
fn directory_filesystem_identity(
    canonical_directory_path: &Path,
    opened_directory_metadata: &Metadata,
) -> Result<DirectoryIdentity, FilesystemError> {
    let file_identity = win_file_id::get_high_res_file_id(canonical_directory_path)
        .map_err(|source| io_error(canonical_directory_path, source))?;

    let win_file_id::FileId::HighRes {
        volume_serial_number,
        file_id,
    } = file_identity
    else {
        return Err(io_error(
            canonical_directory_path,
            Error::new(
                ErrorKind::Unsupported,
                "high-resolution directory identity is unavailable",
            ),
        ));
    };

    let file_identifier_bytes = file_id.to_le_bytes();

    let opened_file_identifier = u64::from_le_bytes([
        file_identifier_bytes[0],
        file_identifier_bytes[1],
        file_identifier_bytes[2],
        file_identifier_bytes[3],
        file_identifier_bytes[4],
        file_identifier_bytes[5],
        file_identifier_bytes[6],
        file_identifier_bytes[7],
    ]);

    let opened_volume_identifier = volume_serial_number & u64::from(u32::MAX);
    let is_directory_changed = opened_directory_metadata.dev() != opened_volume_identifier
        || opened_directory_metadata.ino() != opened_file_identifier;

    if is_directory_changed {
        return Err(io_error(
            canonical_directory_path,
            Error::new(ErrorKind::InvalidData, "directory identity changed while it was opened"),
        ));
    }

    Ok(DirectoryIdentity::Windows {
        volume_identifier: volume_serial_number.to_le_bytes().to_vec(),
        file_identifier: file_id.to_le_bytes(),
    })
}

impl WorkspaceRelativePath {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, FilesystemError> {
        let path = path.as_ref();
        let valid = !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));

        if !valid {
            return Err(FilesystemError::InvalidWorkspaceRelativePath { path: path.to_owned() });
        }

        Ok(Self(path.to_owned()))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

fn map_open_error(path: &Path, source: Error) -> FilesystemError {
    if source.raw_os_error() == Some(40) {
        FilesystemError::SymbolicLinkNotAllowed { path: path.to_owned() }
    } else {
        io_error(path, source)
    }
}

fn io_error(path: impl Into<PathBuf>, source: Error) -> FilesystemError {
    FilesystemError::from_input_output_operation(path, source)
}
