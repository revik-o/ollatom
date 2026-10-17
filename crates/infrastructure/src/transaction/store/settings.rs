use crate::InfrastructureResult;
use crate::database::{database_operation_error, format_timestamp, validate_usage_percentage};
use sqlx::{Sqlite, Transaction};
use time::OffsetDateTime;

#[derive(Clone, Copy)]
pub(crate) enum BooleanEntitySetting {
    ProjectThinking,
    ProjectContextOptimization,
    ChatThinking,
    ChatContextOptimization,
}

impl BooleanEntitySetting {
    const fn update_statement(self) -> &'static str {
        match self {
            Self::ProjectThinking => "UPDATE projects SET llm_thinking_enabled = ?, updated_at = ? WHERE id = ?",
            Self::ProjectContextOptimization => {
                "UPDATE projects SET llm_context_optimization_enabled = ?, updated_at = ? WHERE id = ?"
            }
            Self::ChatThinking => "UPDATE chats SET llm_thinking_enabled = ?, updated_at = ? WHERE id = ?",
            Self::ChatContextOptimization => {
                "UPDATE chats SET llm_context_optimization_enabled = ?, updated_at = ? WHERE id = ?"
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum UsageEntitySetting {
    ProjectCpu,
    ProjectGpu,
    ChatCpu,
    ChatGpu,
}

impl UsageEntitySetting {
    const fn update_statement(self) -> &'static str {
        match self {
            Self::ProjectCpu => "UPDATE projects SET cpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
            Self::ProjectGpu => "UPDATE projects SET gpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
            Self::ChatCpu => "UPDATE chats SET cpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
            Self::ChatGpu => "UPDATE chats SET gpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
        }
    }

    const fn field_name(self) -> &'static str {
        match self {
            Self::ProjectCpu | Self::ChatCpu => "CPU usage percentage",
            Self::ProjectGpu | Self::ChatGpu => "GPU usage percentage",
        }
    }
}

pub(crate) async fn set_boolean_entity_value(
    database_transaction: &mut Transaction<'static, Sqlite>,
    setting: BooleanEntitySetting,
    column_value: bool,
    entity_id: Vec<u8>,
) -> InfrastructureResult<bool> {
    let updated_at = format_timestamp(OffsetDateTime::now_utc())?;
    let query_result = sqlx::query(setting.update_statement())
        .bind(column_value)
        .bind(updated_at)
        .bind(entity_id)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to update boolean setting", source))?;

    Ok(query_result.rows_affected() > 0)
}

pub(crate) async fn set_usage_entity_value(
    database_transaction: &mut Transaction<'static, Sqlite>,
    setting: UsageEntitySetting,
    column_value: u8,
    entity_id: Vec<u8>,
) -> InfrastructureResult<bool> {
    validate_usage_percentage(setting.field_name(), column_value)?;

    let updated_at = format_timestamp(OffsetDateTime::now_utc())?;
    let query_result = sqlx::query(setting.update_statement())
        .bind(i64::from(column_value))
        .bind(updated_at)
        .bind(entity_id)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to update usage setting", source))?;

    Ok(query_result.rows_affected() > 0)
}
