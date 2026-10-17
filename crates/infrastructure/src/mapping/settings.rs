use super::columns::{read_integer_boolean, read_u8};
use crate::{ExecutionSettings, InfrastructureResult};
use sqlx::sqlite::SqliteRow;

pub(super) fn map_execution_settings(database_row: &SqliteRow) -> InfrastructureResult<ExecutionSettings> {
    Ok(ExecutionSettings {
        large_language_model_thinking_enabled: read_integer_boolean(database_row, "llm_thinking_enabled")?,
        context_optimization_enabled: read_integer_boolean(database_row, "llm_context_optimization_enabled")?,
        cpu_usage_percentage: read_u8(database_row, "cpu_usage_percentage")?,
        gpu_usage_percentage: read_u8(database_row, "gpu_usage_percentage")?,
    })
}
