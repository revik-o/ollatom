use crate::FilesystemError;
use atomic_write_file::AtomicWriteFile;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePointer {
    path: PathBuf,
}

impl FilePointer {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn read_bytes(&self) -> Result<Vec<u8>, FilesystemError> {
        fs::read(&self.path)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))
    }

    pub async fn read_text(&self) -> Result<String, FilesystemError> {
        fs::read_to_string(&self.path)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))
    }

    pub async fn write_bytes(&self, contents: &[u8]) -> Result<&Self, FilesystemError> {
        fs::write(&self.path, contents)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))?;
        Ok(self)
    }

    pub async fn write_text(&self, contents: &str) -> Result<&Self, FilesystemError> {
        self.write_bytes(contents.as_bytes()).await
    }

    pub async fn append_bytes(&self, contents: &[u8]) -> Result<&Self, FilesystemError> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))?;

        file.write_all(contents)
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))?;
        file.flush()
            .await
            .map_err(|source| FilesystemError::from_input_output_operation(&self.path, source))?;
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
    fs::create_dir_all(directory_path)
        .await
        .map_err(|source| FilesystemError::from_input_output_operation(directory_path, source))?;

    let file_path = create_entry_path(file_name.as_ref(), directory_path)?;
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&file_path)
        .await
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

pub(crate) async fn write_file_atomically(
    file_path: impl AsRef<Path>,
    contents: impl Into<Vec<u8>>,
) -> Result<(), FilesystemError> {
    let file_path = file_path.as_ref().to_owned();
    let filesystem_operation_path = file_path.clone();
    let contents = contents.into();

    tokio::task::spawn_blocking(move || {
        let mut atomic_file =
            AtomicWriteFile::open(&filesystem_operation_path).map_err(|source| {
                FilesystemError::from_input_output_operation(&filesystem_operation_path, source)
            })?;
        atomic_file.write_all(&contents).map_err(|source| {
            FilesystemError::from_input_output_operation(&filesystem_operation_path, source)
        })?;
        atomic_file.commit().map_err(|source| {
            FilesystemError::from_input_output_operation(&filesystem_operation_path, source)
        })
    })
    .await
    .map_err(|source| FilesystemError::FilesystemOperationTaskFailed {
        path: file_path,
        source,
    })?
}
