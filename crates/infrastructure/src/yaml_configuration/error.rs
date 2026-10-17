use std::path::PathBuf;
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum YamlConfigurationError {
    #[error(transparent)]
    Filesystem(#[from] filesystem::FilesystemError),
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
    #[error("failed to coordinate YAML configuration access for '{path}': {source}")]
    Coordination {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("YAML configuration commit registry for '{path}' is full (capacity {capacity})")]
    ConfigurationCommitRegistryFull { path: PathBuf, capacity: usize },
    #[error("YAML configuration commit identifiers are exhausted for '{path}'")]
    ConfigurationCommitIdentifierExhausted { path: PathBuf },
    #[error("YAML configuration commit {operation_identifier} failed: {message}")]
    ConfigurationCommitFailed { operation_identifier: u64, message: String },
    #[error("YAML coordination task for '{path}' failed: {source}")]
    CoordinationTaskFailed {
        path: PathBuf,
        #[source]
        source: tokio::task::JoinError,
    },
}
