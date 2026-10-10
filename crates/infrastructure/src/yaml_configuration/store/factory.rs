use super::super::YamlConfigurationError;
use super::super::document::read_configuration_document;
use super::coordinator::configuration_coordinator;
use super::{ConfigurationFile, JsonConfigurationStore, TypedYamlFile};
use filesystem::{FilePointer, create_file};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::path::Path;

pub async fn create_json_configuration_store(
    file_name: impl AsRef<str>,
    directory_path: impl AsRef<Path>,
) -> Result<JsonConfigurationStore, YamlConfigurationError> {
    let file = open_configuration_file(file_name, directory_path).await?;
    read_configuration_document(&file.file_pointer).await?;

    Ok(JsonConfigurationStore { file })
}

pub async fn create_typed_yaml_file<Configuration>(
    file_name: impl AsRef<str>,
    directory_path: impl AsRef<Path>,
) -> Result<TypedYamlFile<Configuration>, YamlConfigurationError>
where
    Configuration: DeserializeOwned + Serialize + Sync,
{
    Ok(TypedYamlFile {
        file: open_configuration_file(file_name, directory_path).await?,
        configuration_marker: PhantomData,
    })
}

async fn open_configuration_file(
    file_name: impl AsRef<str>,
    directory_path: impl AsRef<Path>,
) -> Result<ConfigurationFile, YamlConfigurationError> {
    let file_name = file_name.as_ref();
    let directory_path = directory_path.as_ref();

    tokio::fs::create_dir_all(directory_path)
        .await
        .map_err(|source| YamlConfigurationError::Coordination {
            path: directory_path.to_owned(),
            source,
        })?;

    filesystem::restrict_directory_to_current_user(directory_path).map_err(|source| {
        YamlConfigurationError::Coordination {
            path: directory_path.to_owned(),
            source,
        }
    })?;

    let _opened_file = create_file(file_name, directory_path).await?;
    let canonical_parent = tokio::fs::canonicalize(directory_path)
        .await
        .map_err(|source| YamlConfigurationError::Coordination {
            path: directory_path.to_owned(),
            source,
        })?;

    let file_path = canonical_parent.join(file_name);
    let lock_file_path = canonical_parent.join(format!("{file_name}.lock"));
    let coordinator = configuration_coordinator(file_path.clone());

    Ok(ConfigurationFile {
        file_pointer: FilePointer::from_path(file_path.clone()),
        file_path,
        lock_file_path,
        coordinator,
    })
}
