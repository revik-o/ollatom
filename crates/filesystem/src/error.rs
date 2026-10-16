use std::path::PathBuf;
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum FilesystemError {
    #[error("invalid filesystem entry name '{entry_name}'")]
    InvalidFilesystemEntryName { entry_name: String },
    #[error("invalid workspace-relative path '{path}'")]
    InvalidWorkspaceRelativePath { path: PathBuf },
    #[error("symbolic links are not allowed at '{path}'")]
    SymbolicLinkNotAllowed { path: PathBuf },
    #[error("'{path}' is not a regular file")]
    NotARegularFile { path: PathBuf },
    #[error("file '{path}' exceeds the {maximum_byte_count}-byte limit")]
    FileSizeLimitExceeded {
        path: PathBuf,
        maximum_byte_count: u64,
    },
    #[error("destination '{path}' already exists")]
    DestinationAlreadyExists { path: PathBuf },
    #[error("atomic metadata policy is not supported for '{path}'")]
    AtomicMetadataPolicyUnsupported { path: PathBuf },
    #[error("atomic commit identifier space is exhausted")]
    AtomicCommitIdentifierExhausted,
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
