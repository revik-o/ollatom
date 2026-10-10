use crate::FilesystemError;
use std::path::{Component, Path, PathBuf};
use tokio::fs::{self, OpenOptions};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePointer {
    path: PathBuf,
}

impl FilePointer {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn read_bytes(&self) -> Result<Vec<u8>, FilesystemError> {
        fs::read(&self.path)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))
    }

    pub async fn read_bytes_bounded(
        &self,
        maximum_byte_count: u64,
    ) -> Result<Vec<u8>, FilesystemError> {
        let parent_path = self.path.parent().unwrap_or_else(|| Path::new("."));

        let file_name =
            self.path
                .file_name()
                .ok_or_else(|| FilesystemError::InvalidWorkspaceRelativePath {
                    path: self.path.clone(),
                })?;

        crate::WorkspaceDirectory::open(parent_path)
            .await?
            .file(Path::new(file_name))?
            .read_regular_file_bytes_bounded(maximum_byte_count)
            .await
    }

    pub async fn read_text_bounded(
        &self,
        maximum_byte_count: u64,
    ) -> Result<String, FilesystemError> {
        let bytes = self.read_bytes_bounded(maximum_byte_count).await?;

        String::from_utf8(bytes).map_err(|source| {
            FilesystemError::from_input_output_operation(
                &self.path,
                std::io::Error::new(std::io::ErrorKind::InvalidData, source),
            )
        })
    }

    pub async fn read_text(&self) -> Result<String, FilesystemError> {
        fs::read_to_string(&self.path)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))
    }

    pub async fn write_bytes(&self, contents: &[u8]) -> Result<&Self, FilesystemError> {
        workspace_file(&self.path)
            .await?
            .write_regular_file_bytes(contents.to_vec())
            .await?;

        Ok(self)
    }

    pub async fn write_text(&self, contents: &str) -> Result<&Self, FilesystemError> {
        self.write_bytes(contents.as_bytes()).await
    }

    pub async fn write_bytes_atomically(
        &self,
        contents: impl Into<Vec<u8>>,
    ) -> Result<&Self, FilesystemError> {
        write_file_atomically(&self.path, contents).await?;
        Ok(self)
    }

    pub async fn write_text_atomically(
        &self,
        contents: impl Into<String>,
    ) -> Result<&Self, FilesystemError> {
        self.write_bytes_atomically(contents.into().into_bytes())
            .await
    }

    pub async fn append_bytes(&self, contents: &[u8]) -> Result<&Self, FilesystemError> {
        workspace_file(&self.path)
            .await?
            .append_regular_file_bytes(contents.to_vec())
            .await?;

        Ok(self)
    }

    pub async fn append_text(&self, contents: &str) -> Result<&Self, FilesystemError> {
        self.append_bytes(contents.as_bytes()).await
    }
}

pub async fn create_folder(
    folder_name: impl AsRef<str>,
    parent_directory_path: impl AsRef<Path>,
) -> Result<PathBuf, FilesystemError> {
    let folder_path = create_entry_path(folder_name.as_ref(), parent_directory_path.as_ref())?;

    fs::create_dir_all(&folder_path)
        .await
        .map_err(|source| FilesystemError::from_input_output_operation(&folder_path, source))?;

    Ok(folder_path)
}

pub async fn create_file(
    file_name: impl AsRef<str>,
    directory_path: impl AsRef<Path>,
) -> Result<FilePointer, FilesystemError> {
    let directory_path = directory_path.as_ref();

    let file_path = create_entry_path(file_name.as_ref(), directory_path)?;

    fs::create_dir_all(directory_path)
        .await
        .map_err(|source| FilesystemError::from_input_output_operation(directory_path, source))?;

    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(false);

    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits());
    #[cfg(windows)]
    options
        .access_mode(
            crate::permissions::windows_file_access::FILE_GENERIC_WRITE
                | crate::permissions::windows_file_access::FILE_READ_ATTRIBUTES
                | crate::permissions::windows_file_access::READ_CONTROL
                | crate::permissions::windows_file_access::WRITE_DAC,
        )
        .share_mode(crate::permissions::windows_file_access::FILE_SHARE_NONE)
        .custom_flags(crate::permissions::windows_file_attribute::OPEN_REPARSE_POINT);

    let file = options
        .open(&file_path)
        .await
        .map_err(|source| FilesystemError::from_input_output_operation(&file_path, source))?;

    let mut file = file.into_std().await;

    crate::permissions::restrict_open_file_to_current_user(&mut file)
        .map_err(|source| FilesystemError::from_input_output_operation(&file_path, source))?;

    Ok(FilePointer { path: file_path })
}

fn create_entry_path(
    entry_name: &str,
    parent_directory_path: &Path,
) -> Result<PathBuf, FilesystemError> {
    let entry_name_path = Path::new(entry_name);
    let mut path_components = entry_name_path.components();
    let first_path_component = path_components.next();
    let has_exactly_one_normal_component =
        matches!(first_path_component, Some(Component::Normal(_)))
            && path_components.next().is_none();

    if entry_name.is_empty() || !has_exactly_one_normal_component {
        return Err(FilesystemError::InvalidFilesystemEntryName {
            entry_name: entry_name.to_owned(),
        });
    }

    Ok(parent_directory_path.join(entry_name_path))
}

async fn write_file_atomically(
    file_path: impl AsRef<Path>,
    contents: impl Into<Vec<u8>>,
) -> Result<(), FilesystemError> {
    let file_path = file_path.as_ref().to_owned();
    let contents = contents.into();
    let commit = workspace_file(&file_path)
        .await?
        .prepare_atomic_write(contents, crate::AtomicMetadataPolicy::PreserveSupported)
        .await?
        .start_commit()?;

    match commit.wait().await {
        crate::AtomicCommitStatus::Succeeded => Ok(()),
        crate::AtomicCommitStatus::Pending => unreachable!(),
        crate::AtomicCommitStatus::Failed(message) => Err(
            FilesystemError::from_input_output_operation(file_path, std::io::Error::other(message)),
        ),
    }
}

async fn workspace_file(path: &Path) -> Result<crate::WorkspaceFile, FilesystemError> {
    let parent_path = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name =
        path.file_name()
            .ok_or_else(|| FilesystemError::InvalidWorkspaceRelativePath {
                path: path.to_owned(),
            })?;

    crate::WorkspaceDirectory::open(parent_path)
        .await?
        .file(Path::new(file_name))
}
