use super::super::YamlConfigurationError;
use super::super::document::{
    add_value_to_configuration_document, read_configuration_document,
    serialize_configuration_document,
};
use super::{ConfigurationCommitIdentifier, ConfigurationCommitStatus, ConfigurationFile};
use filesystem::FilePointer;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{OwnedMutexGuard, watch};

impl ConfigurationFile {
    pub(super) fn commit_status(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
    ) -> Option<ConfigurationCommitStatus> {
        self.coordinator
            .commit_registry
            .status(operation_identifier, &self.file_path)
            .ok()
            .flatten()
    }

    pub(super) fn acknowledge_commit(
        &self,
        operation_identifier: ConfigurationCommitIdentifier,
    ) -> Option<ConfigurationCommitStatus> {
        self.coordinator
            .commit_registry
            .acknowledge(operation_identifier, &self.file_path)
            .ok()
            .flatten()
    }

    pub(super) fn register_commit(
        &self,
    ) -> Result<
        (
            ConfigurationCommitIdentifier,
            watch::Sender<ConfigurationCommitStatus>,
            watch::Receiver<ConfigurationCommitStatus>,
        ),
        YamlConfigurationError,
    > {
        self.coordinator.commit_registry.register(&self.file_path)
    }

    async fn acquire_mutation_locks(
        &self,
    ) -> Result<(OwnedMutexGuard<()>, File), YamlConfigurationError> {
        let coordinator_guard = Arc::clone(&self.coordinator.mutation_lock)
            .lock_owned()
            .await;

        let lock_file_path = self.lock_file_path.clone();
        let lock_file = tokio::task::spawn_blocking(move || {
            let lock_file = open_private_configuration_lock_file(&lock_file_path)?;
            lock_file
                .lock()
                .map_err(|source| YamlConfigurationError::Coordination {
                    path: lock_file_path,
                    source,
                })?;
            Ok::<File, YamlConfigurationError>(lock_file)
        })
        .await
        .map_err(|source| YamlConfigurationError::CoordinationTaskFailed {
            path: self.lock_file_path.clone(),
            source,
        })??;

        Ok((coordinator_guard, lock_file))
    }

    pub(super) async fn apply_parameters(
        &self,
        parameters: BTreeMap<String, Value>,
    ) -> Result<(), YamlConfigurationError> {
        let (_coordinator_guard, _lock_file) = self.acquire_mutation_locks().await?;
        let mut configuration_document = read_configuration_document(&self.file_pointer).await?;

        for (configuration_key, configuration_value) in parameters {
            add_value_to_configuration_document(
                &mut configuration_document,
                &configuration_key,
                configuration_value,
            )?;
        }

        persist_configuration_document(&self.file_pointer, &configuration_document).await
    }

    pub(super) async fn replace_contents(
        &self,
        configuration_contents: String,
    ) -> Result<(), YamlConfigurationError> {
        let (_coordinator_guard, _lock_file) = self.acquire_mutation_locks().await?;

        self.refresh_current_file().await?;
        self.file_pointer
            .write_text_atomically(configuration_contents)
            .await?;

        Ok(())
    }

    async fn refresh_current_file(&self) -> Result<(), YamlConfigurationError> {
        let _ = self.file_pointer.read_text().await?;
        Ok(())
    }
}

fn open_private_configuration_lock_file(
    lock_file_path: &Path,
) -> Result<File, YamlConfigurationError> {
    filesystem::open_or_create_owner_only_file(lock_file_path).map_err(|source| {
        YamlConfigurationError::Coordination {
            path: lock_file_path.to_owned(),
            source,
        }
    })
}

async fn persist_configuration_document(
    file_pointer: &FilePointer,
    configuration_document: &Value,
) -> Result<(), YamlConfigurationError> {
    let configuration_contents =
        serialize_configuration_document(configuration_document, file_pointer.path())?;

    file_pointer
        .write_text_atomically(configuration_contents)
        .await?;

    Ok(())
}
