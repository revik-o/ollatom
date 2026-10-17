use super::super::YamlConfigurationError;
use super::super::document::{
    read_configuration_document, read_value_from_configuration_document, validate_configuration_key,
};
use super::{
    ConfigurationCommitHandle, ConfigurationCommitIdentifier, ConfigurationCommitStatus, JsonConfigurationStore,
    JsonConfigurationUpdate,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

impl JsonConfigurationStore {
    pub fn file_path(&self) -> &Path {
        &self.file.file_path
    }

    pub fn create_update(&self) -> JsonConfigurationUpdate {
        JsonConfigurationUpdate {
            configuration_store: self.clone(),
            parameters: BTreeMap::new(),
        }
    }

    pub fn add_parameter(
        self,
        configuration_key: impl Into<String>,
        configuration_value: impl Into<Value>,
    ) -> Result<JsonConfigurationUpdate, YamlConfigurationError> {
        self.create_update()
            .add_parameter(configuration_key, configuration_value)
    }

    pub async fn read_parameter(&self, configuration_key: &str) -> Result<Option<Value>, YamlConfigurationError> {
        validate_configuration_key(configuration_key)?;
        let _coordinator_guard = self.file.coordinator.mutation_lock.lock().await;
        let configuration_document = read_configuration_document(&self.file.file_pointer).await?;

        Ok(read_value_from_configuration_document(
            &configuration_document,
            configuration_key,
        ))
    }

    pub fn commit_status(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
    ) -> Option<ConfigurationCommitStatus> {
        self.file.commit_status(operation_identifier)
    }

    pub fn acknowledge_commit(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
    ) -> Option<ConfigurationCommitStatus> {
        self.file.acknowledge_commit(operation_identifier)
    }
}
impl JsonConfigurationUpdate {
    pub fn add_parameter(
        mut self,
        configuration_key: impl Into<String>,
        configuration_value: impl Into<Value>,
    ) -> Result<Self, YamlConfigurationError> {
        let configuration_key = configuration_key.into();

        validate_configuration_key(&configuration_key)?;

        self.parameters.insert(configuration_key, configuration_value.into());

        Ok(self)
    }

    pub fn commit(self) -> Result<ConfigurationCommitHandle, YamlConfigurationError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|source| YamlConfigurationError::Coordination {
            path: self.configuration_store.file.file_path.clone(),
            source: std::io::Error::other(source),
        })?;

        let configuration_file = self.configuration_store.file.clone();
        let (operation_identifier, completion_sender, completion_receiver) = configuration_file.register_commit()?;

        runtime.spawn(async move {
            let status = match configuration_file.apply_parameters(self.parameters).await {
                Ok(()) => ConfigurationCommitStatus::Succeeded,
                Err(error) => ConfigurationCommitStatus::Failed(error.to_string()),
            };
            completion_sender.send_replace(status);
        });

        Ok(ConfigurationCommitHandle {
            operation_identifier,
            completion_receiver,
            coordinator: Arc::clone(&self.configuration_store.file.coordinator),
        })
    }
}
