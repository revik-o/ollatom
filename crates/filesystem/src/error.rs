use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilesystemError {
    #[error("invalid filesystem entry name '{entry_name}'")]
    InvalidFilesystemEntryName { entry_name: String },
    #[error("filesystem operation failed for '{path}': {source}")]
    InputOutputOperation {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("filesystem operation task failed for '{path}': {source}")]
    FilesystemOperationTaskFailed {
        path: PathBuf,
        #[source]
        source: tokio::task::JoinError,
    },
}

impl FilesystemError {
    pub(crate) fn from_input_output_operation(
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::InputOutputOperation {
            path: path.into(),
            source,
        }
    }
}
