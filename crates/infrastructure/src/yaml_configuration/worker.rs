use super::YamlConfigurationError;
use super::document::{
    add_value_to_configuration_document, read_value_from_configuration_document,
    serialize_configuration_document, validate_configuration_document_root,
};
use filesystem::FilePointer;
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::{mpsc::Receiver, oneshot::Sender};

pub(super) enum YamlConfigurationCommand {
    ReadParameter {
        configuration_key: String,
        result_sender: Sender<Result<Option<Value>, YamlConfigurationError>>,
    },
    ReadConfigurationDocument {
        result_sender: Sender<Result<Value, YamlConfigurationError>>,
    },
    CommitParameters {
        parameters: BTreeMap<String, Value>,
        result_sender: Sender<Result<(), YamlConfigurationError>>,
    },
    ReplaceConfigurationDocument {
        configuration_document: Value,
        result_sender: Sender<Result<(), YamlConfigurationError>>,
    },
}

async fn replace_configuration_document(
    file_pointer: &FilePointer,
    replacement_configuration_document: Value,
) -> Result<Value, YamlConfigurationError> {
    validate_configuration_document_root(&replacement_configuration_document, file_pointer.path())?;
    persist_configuration_document(file_pointer, &replacement_configuration_document).await?;
    Ok(replacement_configuration_document)
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
                let command_result = Ok(read_value_from_configuration_document(
                    &configuration_document,
                    &configuration_key,
                ));
                let _send_result = result_sender.send(command_result);
            }
            YamlConfigurationCommand::ReadConfigurationDocument { result_sender } => {
                let _send_result = result_sender.send(Ok(configuration_document.clone()));
            }
            YamlConfigurationCommand::CommitParameters {
                parameters,
                result_sender,
            } => {
                let command_result =
                    commit_parameters(&file_pointer, &configuration_document, parameters)
                        .await
                        .map(|updated_configuration_document| {
                            configuration_document = updated_configuration_document;
                        });
                let _send_result = result_sender.send(command_result);
            }
            YamlConfigurationCommand::ReplaceConfigurationDocument {
                configuration_document: replacement_configuration_document,
                result_sender,
            } => {
                let command_result = replace_configuration_document(
                    &file_pointer,
                    replacement_configuration_document,
                )
                .await
                .map(|updated_configuration_document| {
                    configuration_document = updated_configuration_document;
                });
                let _send_result = result_sender.send(command_result);
            }
        }
    }
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
    configuration_document: &Value,
    parameters: BTreeMap<String, Value>,
) -> Result<Value, YamlConfigurationError> {
    let mut updated_configuration_document = configuration_document.clone();

    for (configuration_key, configuration_value) in parameters {
        add_value_to_configuration_document(
            &mut updated_configuration_document,
            &configuration_key,
            configuration_value,
        )?;
    }

    persist_configuration_document(file_pointer, &updated_configuration_document).await?;
    Ok(updated_configuration_document)
}
