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
    #[error("invalid YAML configuration key '{configuration_key}'")]
    InvalidYamlConfigurationKey { configuration_key: String },
    #[error("YAML configuration key '{configuration_key}' conflicts with an existing value")]
    YamlConfigurationKeyConflict { configuration_key: String },
    #[error("failed to deserialize YAML file '{path}': {source}")]
    YamlDeserialization {
        path: PathBuf,
        #[source]
        source: Box<serde_saphyr::DeserializeError>,
    },
    #[error("failed to serialize YAML file '{path}': {source}")]
    YamlSerialization {
        path: PathBuf,
        #[source]
        source: Box<serde_saphyr::SerializeError>,
    },
    #[error("YAML document '{path}' must contain a mapping at its root")]
    YamlRootIsNotMapping { path: PathBuf },
    #[error("YAML configuration worker for '{path}' is unavailable")]
    YamlConfigurationWorkerUnavailable { path: PathBuf },
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
