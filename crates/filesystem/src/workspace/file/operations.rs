use super::super::{WorkspaceFile, WorkspaceRelativePath, io_error, map_open_error};
use crate::FilesystemError;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Component;

impl WorkspaceFile {
    pub fn relative_path(&self) -> &WorkspaceRelativePath {
        &self.relative_path
    }

    pub async fn read_regular_file_bytes_bounded(
        &self,
        maximum_byte_count: u64,
    ) -> Result<Vec<u8>, FilesystemError> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.read_bounded_blocking(maximum_byte_count))
            .await
            .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
                path: self.relative_path.0.clone(),
                source,
            })?
    }

    pub async fn read_regular_file_text_bounded(
        &self,
        maximum_byte_count: u64,
    ) -> Result<String, FilesystemError> {
        let bytes = self
            .read_regular_file_bytes_bounded(maximum_byte_count)
            .await?;

        String::from_utf8(bytes).map_err(|source| {
            io_error(
                &self.relative_path.0,
                std::io::Error::new(std::io::ErrorKind::InvalidData, source),
            )
        })
    }

    pub async fn write_regular_file_bytes(&self, contents: Vec<u8>) -> Result<(), FilesystemError> {
        self.write_regular_file_bytes_with_mode(contents, false)
            .await
    }

    pub async fn append_regular_file_bytes(
        &self,
        contents: Vec<u8>,
    ) -> Result<(), FilesystemError> {
        self.write_regular_file_bytes_with_mode(contents, true)
            .await
    }

    pub async fn create_directory_all(&self) -> Result<(), FilesystemError> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut directory = this
                .workspace_directory
                .directory
                .try_clone()
                .map_err(|source| io_error(&this.relative_path.0, source))?;

            for component in this.relative_path.0.components() {
                let Component::Normal(name) = component else {
                    return Err(FilesystemError::InvalidWorkspaceRelativePath {
                        path: this.relative_path.0.clone(),
                    });
                };

                match directory.open_dir_nofollow(name) {
                    Ok(child) => directory = child,
                    Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                        directory
                            .create_dir(name)
                            .map_err(|source| io_error(&this.relative_path.0, source))?;
                        directory = directory
                            .open_dir_nofollow(name)
                            .map_err(|source| map_open_error(&this.relative_path.0, source))?;
                    }
                    Err(source) => {
                        return Err(map_open_error(&this.relative_path.0, source));
                    }
                }
            }

            Ok(())
        })
        .await
        .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
            path: self.relative_path.0.clone(),
            source,
        })?
    }

    pub async fn remove_entry(&self) -> Result<(), FilesystemError> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let (parent, name) = this.open_parent()?;
            let metadata = parent
                .symlink_metadata(&name)
                .map_err(|source| io_error(&this.relative_path.0, source))?;

            if metadata.file_type().is_symlink() {
                return Err(FilesystemError::SymbolicLinkNotAllowed {
                    path: this.relative_path.0.clone(),
                });
            }

            if metadata.is_dir() {
                parent
                    .remove_dir_all(&name)
                    .map_err(|source| io_error(&this.relative_path.0, source))
            } else {
                parent
                    .remove_file(&name)
                    .map_err(|source| io_error(&this.relative_path.0, source))
            }
        })
        .await
        .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
            path: self.relative_path.0.clone(),
            source,
        })?
    }
    pub async fn rename_without_replacement(
        &self,
        destination: &WorkspaceFile,
    ) -> Result<(), FilesystemError> {
        let source = self.clone();
        let destination = destination.clone();

        tokio::task::spawn_blocking(move || {
            let (source_parent, source_name) = source.open_parent()?;
            let (destination_parent, destination_name) = destination.open_parent()?;

            source_parent
                .hard_link(&source_name, &destination_parent, &destination_name)
                .map_err(|source_error| {
                    if source_error.kind() == std::io::ErrorKind::AlreadyExists {
                        FilesystemError::DestinationAlreadyExists {
                            path: destination.relative_path.0.clone(),
                        }
                    } else {
                        io_error(&destination.relative_path.0, source_error)
                    }
                })?;

            if let Err(source_error) = source_parent.remove_file(&source_name) {
                let _ = destination_parent.remove_file(&destination_name);
                return Err(io_error(&source.relative_path.0, source_error));
            }

            Ok(())
        })
        .await
        .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
            path: self.relative_path.0.clone(),
            source,
        })?
    }
    fn read_bounded_blocking(&self, maximum_byte_count: u64) -> Result<Vec<u8>, FilesystemError> {
        let (parent, name) = self.open_parent()?;
        let mut options = OpenOptions::new();

        options.read(true).follow(FollowSymlinks::No);

        let mut file = parent
            .open_with(&name, &options)
            .map_err(|source| map_open_error(&self.relative_path.0, source))?;

        let metadata = file
            .metadata()
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        if !metadata.is_file() {
            return Err(FilesystemError::NotARegularFile {
                path: self.relative_path.0.clone(),
            });
        }

        let mut bytes = Vec::new();

        Read::by_ref(&mut file)
            .take(maximum_byte_count.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_byte_count {
            return Err(FilesystemError::FileSizeLimitExceeded {
                path: self.relative_path.0.clone(),
                maximum_byte_count,
            });
        }

        Ok(bytes)
    }

    async fn write_regular_file_bytes_with_mode(
        &self,
        contents: Vec<u8>,
        append: bool,
    ) -> Result<(), FilesystemError> {
        let this = self.clone();

        tokio::task::spawn_blocking(move || {
            let (parent, name) = this.open_parent()?;
            let mut options = OpenOptions::new();

            options
                .write(true)
                .append(append)
                .truncate(!append)
                .follow(FollowSymlinks::No);

            let mut file = parent
                .open_with(&name, &options)
                .map_err(|source| map_open_error(&this.relative_path.0, source))?;

            let metadata = file
                .metadata()
                .map_err(|source| io_error(&this.relative_path.0, source))?;

            if !metadata.is_file() {
                return Err(FilesystemError::NotARegularFile {
                    path: this.relative_path.0.clone(),
                });
            }

            file.write_all(&contents)
                .and_then(|()| file.sync_all())
                .map_err(|source| io_error(&this.relative_path.0, source))
        })
        .await
        .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
            path: self.relative_path.0.clone(),
            source,
        })?
    }
    pub(super) fn open_parent(&self) -> Result<(Dir, OsString), FilesystemError> {
        let mut parent = self
            .workspace_directory
            .directory
            .try_clone()
            .map_err(|source| io_error(&self.relative_path.0, source))?;

        let mut components = self.relative_path.0.components().peekable();

        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(FilesystemError::InvalidWorkspaceRelativePath {
                    path: self.relative_path.0.clone(),
                });
            };

            if components.peek().is_none() {
                return Ok((parent, name.to_owned()));
            }

            parent = parent
                .open_dir_nofollow(name)
                .map_err(|source| map_open_error(&self.relative_path.0, source))?;
        }
        Err(FilesystemError::InvalidWorkspaceRelativePath {
            path: self.relative_path.0.clone(),
        })
    }
}
