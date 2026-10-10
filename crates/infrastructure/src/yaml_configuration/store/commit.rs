use super::super::YamlConfigurationError;
use super::{
    ConfigurationCommitHandle, ConfigurationCommitIdentifier, ConfigurationCommitRegistry,
    ConfigurationCommitStatus,
};
use std::path::Path;
use tokio::sync::watch;

pub(crate) mod identifiers;

pub(super) use identifiers::next_configuration_commit_identifier;
pub(crate) const MAX_UNACKNOWLEDGED_CONFIGURATION_COMMITS: usize = 1_024;

impl ConfigurationCommitHandle {
    pub fn operation_identifier(&self) -> ConfigurationCommitIdentifier {
        self.operation_identifier
    }

    pub fn status(&self) -> ConfigurationCommitStatus {
        self.completion_receiver.borrow().clone()
    }

    pub async fn wait(mut self) -> ConfigurationCommitStatus {
        while *self.completion_receiver.borrow() == ConfigurationCommitStatus::Pending {
            if self.completion_receiver.changed().await.is_err() {
                break;
            }
        }

        let status = self.completion_receiver.borrow().clone();

        self.coordinator
            .commit_registry
            .acknowledge_identifier(self.operation_identifier);

        status
    }
}
impl ConfigurationCommitRegistry {
    pub(crate) fn register(
        &self,
        file_path: &Path,
    ) -> Result<
        (
            ConfigurationCommitIdentifier,
            watch::Sender<ConfigurationCommitStatus>,
            watch::Receiver<ConfigurationCommitStatus>,
        ),
        YamlConfigurationError,
    > {
        let mut statuses =
            self.statuses
                .lock()
                .map_err(|_| YamlConfigurationError::Coordination {
                    path: file_path.to_owned(),
                    source: std::io::Error::other("commit result registry is poisoned"),
                })?;

        if statuses.len() >= MAX_UNACKNOWLEDGED_CONFIGURATION_COMMITS {
            return Err(YamlConfigurationError::ConfigurationCommitRegistryFull {
                path: file_path.to_owned(),
                capacity: MAX_UNACKNOWLEDGED_CONFIGURATION_COMMITS,
            });
        }

        let operation_identifier = next_configuration_commit_identifier(file_path)?;
        let (completion_sender, completion_receiver) =
            watch::channel(ConfigurationCommitStatus::Pending);

        statuses.insert(operation_identifier, completion_receiver.clone());

        Ok((operation_identifier, completion_sender, completion_receiver))
    }

    pub(super) fn status(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
        file_path: &Path,
    ) -> Result<Option<ConfigurationCommitStatus>, YamlConfigurationError> {
        let statuses = self
            .statuses
            .lock()
            .map_err(|_| YamlConfigurationError::Coordination {
                path: file_path.to_owned(),
                source: std::io::Error::other("commit result registry is poisoned"),
            })?;

        Ok(statuses
            .get(&operation_identifier)
            .map(|receiver| receiver.borrow().clone()))
    }

    pub(crate) fn acknowledge(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
        file_path: &Path,
    ) -> Result<Option<ConfigurationCommitStatus>, YamlConfigurationError> {
        let mut statuses =
            self.statuses
                .lock()
                .map_err(|_| YamlConfigurationError::Coordination {
                    path: file_path.to_owned(),
                    source: std::io::Error::other("commit result registry is poisoned"),
                })?;

        let Some(status) = statuses
            .get(&operation_identifier)
            .map(|receiver| receiver.borrow().clone())
        else {
            return Ok(None);
        };

        if status != ConfigurationCommitStatus::Pending {
            statuses.remove(&operation_identifier);
        }

        Ok(Some(status))
    }

    pub(super) fn acknowledge_identifier(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
    ) {
        let Ok(mut statuses) = self.statuses.lock() else {
            return;
        };
        let is_resolved = statuses
            .get(&operation_identifier)
            .is_some_and(|receiver| *receiver.borrow() != ConfigurationCommitStatus::Pending);

        if is_resolved {
            statuses.remove(&operation_identifier);
        }
    }
}
