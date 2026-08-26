use std::path::PathBuf;
use thiserror::Error;

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
    #[error("YAML configuration worker for '{path}' is unavailable")]
    YamlConfigurationWorkerUnavailable { path: PathBuf },
}
