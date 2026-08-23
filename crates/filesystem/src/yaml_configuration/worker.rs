use super::document::{
    add_value_to_configuration_document, read_value_from_configuration_document,
    serialize_configuration_document, validate_configuration_document_root,
};
use crate::file::write_file_atomically;
use crate::{FilePointer, FilesystemError};
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::{mpsc::Receiver, oneshot::Sender};

pub(super) enum YamlConfigurationCommand {
    ReadParameter {
        configuration_key: String,
        result_sender: Sender<Result<Option<Value>, FilesystemError>>,
    },
    ReadConfigurationDocument {
        result_sender: Sender<Result<Value, FilesystemError>>,
    },
    CommitParameters {
        parameters: BTreeMap<String, Value>,
        result_sender: Sender<Result<(), FilesystemError>>,
    },
    ReplaceConfigurationDocument {
        configuration_document: Value,
        result_sender: Sender<Result<(), FilesystemError>>,
    },
}

// TODO think about this. Should be better solution
async fn replace_configuration_document(
    file_pointer: &FilePointer,
    configuration_document: &mut Value,
    replacement_configuration_document: Value,
) -> Result<(), FilesystemError> {
    validate_configuration_document_root(&replacement_configuration_document, file_pointer.path())?;
    persist_configuration_document(file_pointer, &replacement_configuration_document).await?;
    *configuration_document = replacement_configuration_document;
    Ok(())
}

async fn process_yaml_configuration_commands(
    file_pointer: FilePointer,
    mut configuration_document: Value,
    mut command_receiver: Receiver<YamlConfigurationCommand>,
) {
    while let Some(command) = command_receiver.recv().await {
        match command {
            YamlConfigurationCommand::ReadParameter {
                configuration_key,
                result_sender,
            } => {
                let result = Ok(read_value_from_configuration_document(
                    &configuration_document,
                    &configuration_key,
                ));
                let _result = result_sender.send(result);
            }
            YamlConfigurationCommand::ReadConfigurationDocument { result_sender } => {
                let _result = result_sender.send(Ok(configuration_document.clone()));
            }
            YamlConfigurationCommand::CommitParameters {
                parameters,
                result_sender,
            } => {
                let result =
                    commit_parameters(&file_pointer, &mut configuration_document, parameters).await;
                let _result = result_sender.send(result);
            }
            YamlConfigurationCommand::ReplaceConfigurationDocument {
                configuration_document: replacement_configuration_document,
                result_sender,
            } => {
                let _result = result_sender.send(
                    replace_configuration_document(
                        &file_pointer,
                        &mut configuration_document,
                        replacement_configuration_document,
                    )
                    .await,
                );
            }
        }
    }
}

async fn persist_configuration_document(
    file_pointer: &FilePointer,
    configuration_document: &Value,
) -> Result<(), FilesystemError> {
    let configuration_contents =
        serialize_configuration_document(configuration_document, file_pointer.path())?;
    write_file_atomically(file_pointer.path(), configuration_contents.into_bytes()).await
}

pub(super) fn start_yaml_configuration_worker(
    file_pointer: FilePointer,
    configuration_document: Value,
    command_receiver: Receiver<YamlConfigurationCommand>,
) {
    tokio::spawn(process_yaml_configuration_commands(
        file_pointer,
        configuration_document,
        command_receiver,
    ));
}

async fn commit_parameters(
    file_pointer: &FilePointer,
    configuration_document: &mut Value,
    parameters: BTreeMap<String, Value>,
) -> Result<(), FilesystemError> {
    let mut updated_configuration_document = configuration_document.clone();

    for (configuration_key, configuration_value) in parameters {
        add_value_to_configuration_document(
            &mut updated_configuration_document,
            &configuration_key,
            configuration_value,
        )?;
    }

    persist_configuration_document(file_pointer, &updated_configuration_document).await?;
    *configuration_document = updated_configuration_document;
    Ok(())
}
