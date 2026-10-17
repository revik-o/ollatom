use infrastructure::{
    ConfigurationCommitStatus, JsonConfigurationStore, YamlConfigurationError, create_json_configuration_store,
    create_typed_yaml_file,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tempfile::tempdir;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct TestConfiguration {
    enabled: bool,
    name: String,
}

async fn open_json_store(directory: &std::path::Path) -> JsonConfigurationStore {
    create_json_configuration_store("application.yaml", directory)
        .await
        .unwrap()
}

#[path = "yaml_configuration/commit_and_security.rs"]
mod commit_and_security;
#[path = "yaml_configuration/store_behavior.rs"]
mod store_behavior;
