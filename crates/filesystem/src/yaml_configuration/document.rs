use crate::{FilePointer, FilesystemError};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::path::Path;

pub(super) fn serialize_yaml_configuration<Configuration>(
    configuration: &Configuration,
    file_path: &Path,
) -> Result<String, FilesystemError>
where
    Configuration: Serialize,
{
    serde_saphyr::to_string(configuration).map_err(|source| FilesystemError::YamlSerialization {
        path: file_path.to_owned(),
        source: Box::new(source),
    })
}

pub(super) fn deserialize_configuration_document(
    configuration_contents: &str,
    file_path: &Path,
) -> Result<Value, FilesystemError> {
    let configuration_document = deserialize_yaml_configuration(configuration_contents, file_path)?;
    validate_configuration_document_root(&configuration_document, file_path)?;
    Ok(configuration_document)
}

pub(super) async fn read_configuration_document(
    file_pointer: &FilePointer,
) -> Result<Value, FilesystemError> {
    let configuration_contents = file_pointer.read_text().await?;

    if configuration_contents.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    deserialize_configuration_document(&configuration_contents, file_pointer.path())
}

pub(super) fn serialize_configuration_document(
    configuration_document: &Value,
    file_path: &Path,
) -> Result<String, FilesystemError> {
    serialize_yaml_configuration(configuration_document, file_path)
}

pub(super) fn deserialize_yaml_configuration<Configuration>(
    configuration_contents: &str,
    file_path: &Path,
) -> Result<Configuration, FilesystemError>
where
    Configuration: DeserializeOwned,
{
    serde_saphyr::from_str(configuration_contents).map_err(|source| {
        FilesystemError::YamlDeserialization {
            path: file_path.to_owned(),
            source: Box::new(source),
        }
    })
}

pub(super) fn validate_configuration_document_root(
    configuration_document: &Value,
    file_path: &Path,
) -> Result<(), FilesystemError> {
    if configuration_document.is_null() || configuration_document.is_object() {
        return Ok(());
    }

    Err(FilesystemError::YamlRootIsNotMapping {
        path: file_path.to_owned(),
    })
}

pub(super) fn validate_configuration_key(configuration_key: &str) -> Result<(), FilesystemError> {
    if configuration_key.is_empty()
        || configuration_key
            .split('.')
            .any(|configuration_key_segment| configuration_key_segment.is_empty())
    {
        return Err(FilesystemError::InvalidYamlConfigurationKey {
            configuration_key: configuration_key.to_owned(),
        });
    }

    Ok(())
}

pub(super) fn add_value_to_configuration_document(
    configuration_document: &mut Value,
    configuration_key: &str,
    configuration_value: Value,
) -> Result<(), FilesystemError> {
    if configuration_document.is_null() {
        *configuration_document = Value::Object(Map::new());
    }

    let configuration_key_segments = configuration_key.split('.').collect::<Vec<_>>();
    let mut current_configuration_value = configuration_document;

    for configuration_key_segment in
        &configuration_key_segments[..configuration_key_segments.len() - 1]
    {
        let current_configuration_mapping = current_configuration_value
            .as_object_mut()
            .ok_or_else(|| FilesystemError::YamlConfigurationKeyConflict {
                configuration_key: configuration_key.to_owned(),
            })?;
        current_configuration_value = current_configuration_mapping
            .entry((*configuration_key_segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));

        if !current_configuration_value.is_object() {
            return Err(FilesystemError::YamlConfigurationKeyConflict {
                configuration_key: configuration_key.to_owned(),
            });
        }
    }

    let final_configuration_key_segment = configuration_key_segments
        .last()
        .expect("validated configuration keys always contain a segment");
    current_configuration_value
        .as_object_mut()
        .ok_or_else(|| FilesystemError::YamlConfigurationKeyConflict {
            configuration_key: configuration_key.to_owned(),
        })?
        .insert(
            (*final_configuration_key_segment).to_owned(),
            configuration_value,
        );
    Ok(())
}

pub(super) fn read_value_from_configuration_document(
    configuration_document: &Value,
    configuration_key: &str,
) -> Option<Value> {
    configuration_key
        .split('.')
        .try_fold(configuration_document, |current_value, key_segment| {
            current_value.as_object()?.get(key_segment)
        })
        .cloned()
}
