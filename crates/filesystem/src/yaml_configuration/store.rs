use super::document::{
    deserialize_configuration_document, deserialize_yaml_configuration,
    read_configuration_document, serialize_configuration_document, serialize_yaml_configuration,
    validate_configuration_key,
};
use super::worker::{YamlConfigurationCommand, start_yaml_configuration_worker};
use crate::{FilesystemError, create_file};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

const YAML_CONFIGURATION_COMMAND_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub struct YamlConfigurationStore {
    file_path: PathBuf,
    command_sender: mpsc::Sender<YamlConfigurationCommand>,
}

#[derive(Debug)]
pub struct YamlConfigurationUpdate {
    configuration_store: YamlConfigurationStore,
    parameters: BTreeMap<String, Value>,
}

impl YamlConfigurationStore {
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn create_update(&self) -> YamlConfigurationUpdate {
        YamlConfigurationUpdate {
            configuration_store: self.clone(),
            parameters: BTreeMap::new(),
        }
    }

    pub fn add_parameter(
        self,
        configuration_key: impl Into<String>,
        configuration_value: impl Into<Value>,
    ) -> Result<YamlConfigurationUpdate, FilesystemError> {
        YamlConfigurationUpdate {
            configuration_store: self,
            parameters: BTreeMap::new(),
        }
        .add_parameter(configuration_key, configuration_value)
    }

    pub async fn read_parameter(
        &self,
        configuration_key: &str,
    ) -> Result<Option<Value>, FilesystemError> {
        validate_configuration_key(configuration_key)?;
        let configuration_key = configuration_key.to_owned();
        self.send_command_and_receive_result(|result_sender| {
            YamlConfigurationCommand::ReadParameter {
                configuration_key,
                result_sender,
            }
        })
        .await
    }

    pub async fn read_yaml_configuration<Configuration>(
        &self,
    ) -> Result<Configuration, FilesystemError>
    where
        Configuration: DeserializeOwned,
    {
        let configuration_document = self
            .send_command_and_receive_result(|result_sender| {
                YamlConfigurationCommand::ReadConfigurationDocument { result_sender }
            })
            .await?;
        let configuration_contents =
            serialize_configuration_document(&configuration_document, self.file_path.as_path())?;
        deserialize_yaml_configuration(&configuration_contents, self.file_path.as_path())
    }

    pub async fn write_yaml_configuration<Configuration>(
        &self,
        configuration: &Configuration,
    ) -> Result<&Self, FilesystemError>
    where
        Configuration: Serialize,
    {
        let configuration_contents = serialize_yaml_configuration(configuration, &self.file_path)?;
        let configuration_document =
            deserialize_configuration_document(&configuration_contents, &self.file_path)?;
        self.send_command_and_receive_result(|result_sender| {
            YamlConfigurationCommand::ReplaceConfigurationDocument {
                configuration_document,
                result_sender,
            }
        })
        .await?;
        Ok(self)
    }

    async fn send_command_and_receive_result<ResultValue>(
        &self,
        create_command: impl FnOnce(
            oneshot::Sender<Result<ResultValue, FilesystemError>>,
        ) -> YamlConfigurationCommand,
    ) -> Result<ResultValue, FilesystemError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.command_sender
            .send(create_command(result_sender))
            .await
            .map_err(|_| FilesystemError::YamlConfigurationWorkerUnavailable {
                path: self.file_path.clone(),
            })?;
        result_receiver
            .await
            .map_err(|_| FilesystemError::YamlConfigurationWorkerUnavailable {
                path: self.file_path.clone(),
            })?
    }
}

impl YamlConfigurationUpdate {
    pub fn add_parameter(
        mut self,
        configuration_key: impl Into<String>,
        configuration_value: impl Into<Value>,
    ) -> Result<Self, FilesystemError> {
        let configuration_key = configuration_key.into();
        validate_configuration_key(&configuration_key)?;
        self.parameters
            .insert(configuration_key, configuration_value.into());
        Ok(self)
    }

    pub async fn commit(self) -> Result<YamlConfigurationStore, FilesystemError> {
        let configuration_store = self.configuration_store;
        configuration_store
            .send_command_and_receive_result(|result_sender| {
                YamlConfigurationCommand::CommitParameters {
                    parameters: self.parameters,
                    result_sender,
                }
            })
            .await?;
        Ok(configuration_store)
    }
}

pub async fn create_yaml_configuration_file(
    file_name: impl AsRef<str>,
    directory_path: impl AsRef<Path>,
) -> Result<YamlConfigurationStore, FilesystemError> {
    let file_pointer = create_file(file_name, directory_path).await?;
    let file_path = file_pointer.path().to_owned();
    let configuration_document = read_configuration_document(&file_pointer).await?;
    let (command_sender, command_receiver) =
        mpsc::channel(YAML_CONFIGURATION_COMMAND_QUEUE_CAPACITY);
    start_yaml_configuration_worker(file_pointer, configuration_document, command_receiver);

    Ok(YamlConfigurationStore {
        file_path,
        command_sender,
    })
}
