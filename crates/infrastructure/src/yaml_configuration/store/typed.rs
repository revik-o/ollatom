use super::super::YamlConfigurationError;
use super::super::document::{deserialize_yaml_configuration, serialize_yaml_configuration};
use super::{ConfigurationCommitHandle, ConfigurationCommitIdentifier, ConfigurationCommitStatus, TypedYamlFile};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::sync::Arc;

impl<Configuration> TypedYamlFile<Configuration>
where
    Configuration: DeserializeOwned + Serialize + Sync,
{
    pub fn file_path(&self) -> &Path {
        &self.file.file_path
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

    pub async fn read(&self) -> Result<Configuration, YamlConfigurationError> {
        let _coordinator_guard = self.file.coordinator.mutation_lock.lock().await;
        let configuration_contents = self.file.file_pointer.read_text().await?;

        deserialize_yaml_configuration(&configuration_contents, &self.file.file_path)
    }

    pub fn start_write(
        &self,
        configuration: &Configuration,
    ) -> Result<ConfigurationCommitHandle, YamlConfigurationError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|source| YamlConfigurationError::Coordination {
            path: self.file.file_path.clone(),
            source: std::io::Error::other(source),
        })?;

        let configuration_contents = serialize_yaml_configuration(configuration, &self.file.file_path)?;
        let (operation_identifier, completion_sender, completion_receiver) = self.file.register_commit()?;
        let configuration_file = self.file.clone();

        runtime.spawn(async move {
            let status = match configuration_file.replace_contents(configuration_contents).await {
                Ok(()) => ConfigurationCommitStatus::Succeeded,
                Err(error) => ConfigurationCommitStatus::Failed(error.to_string()),
            };
            completion_sender.send_replace(status);
        });

        Ok(ConfigurationCommitHandle {
            operation_identifier,
            completion_receiver,
            coordinator: Arc::clone(&self.file.coordinator),
        })
    }

    pub async fn write(&self, configuration: &Configuration) -> Result<(), YamlConfigurationError> {
        let handle = self.start_write(configuration)?;
        let operation_identifier = handle.operation_identifier();

        match handle.wait().await {
            ConfigurationCommitStatus::Succeeded => Ok(()),
            ConfigurationCommitStatus::Pending => unreachable!(),
            ConfigurationCommitStatus::Failed(message) => Err(YamlConfigurationError::ConfigurationCommitFailed {
                operation_identifier: operation_identifier.0,
                message,
            }),
        }
    }
}
