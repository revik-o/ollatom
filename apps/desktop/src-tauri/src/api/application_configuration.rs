use filesystem::{YamlConfigurationStore, create_yaml_configuration_file};
use serde::Serialize;
use serde_json::Value;
use std::error::Error;
use tauri::{Manager, State};

const APPLICATION_CONFIGURATION_FILE_NAME: &str = "application.yaml";

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationConfigurationUpdateStatus {
    Success,
}

pub fn initialize(application: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let application_configuration_directory_path = application.path().app_config_dir()?;
    let application_configuration_store =
        tauri::async_runtime::block_on(create_yaml_configuration_file(
            APPLICATION_CONFIGURATION_FILE_NAME,
            application_configuration_directory_path,
        ))?;
    application.manage(application_configuration_store);
    Ok(())
}

#[tauri::command]
pub async fn get_application_config_value_by_key(
    application_configuration_store: State<'_, YamlConfigurationStore>,
    key: String,
) -> Result<Value, String> {
    application_configuration_store
        .read_parameter(&key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("application configuration key '{key}' does not exist"))
        .and_then(validate_application_configuration_value)
}

#[tauri::command]
pub async fn set_application_config_value(
    application_configuration_store: State<'_, YamlConfigurationStore>,
    key: String,
    value: Value,
) -> Result<ApplicationConfigurationUpdateStatus, String> {
    let value = validate_application_configuration_value(value)?;
    application_configuration_store
        .create_update()
        .add_parameter(key, value)
        .map_err(|error| error.to_string())?
        .commit()
        .await
        .map_err(|error| error.to_string())?;
    Ok(ApplicationConfigurationUpdateStatus::Success)
}

pub fn validate_application_configuration_value(value: Value) -> Result<Value, String> {
    if value.is_string() || value.is_number() {
        return Ok(value);
    }

    Err("application configuration values must be strings or numbers".to_owned())
}
