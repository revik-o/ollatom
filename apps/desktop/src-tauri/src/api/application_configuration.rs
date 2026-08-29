use crate::api::application_startup::StartupState;
use serde::Serialize;
use serde_json::Value;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationConfigurationUpdateStatus {
    Success,
}

#[tauri::command]
pub async fn get_application_config_value_by_key(
    startup: State<'_, StartupState>,
    key: String,
) -> Result<Value, String> {
    startup
        .resources()
        .await?
        .configuration
        .read_parameter(&key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("application configuration key '{key}' does not exist"))
        .and_then(validate_application_configuration_value)
}

#[tauri::command]
pub async fn set_application_config_value(
    startup: State<'_, StartupState>,
    key: String,
    value: Value,
) -> Result<ApplicationConfigurationUpdateStatus, String> {
    let value = validate_application_configuration_value(value)?;

    startup
        .resources()
        .await?
        .configuration
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
