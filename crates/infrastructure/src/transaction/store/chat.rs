use super::super::CHAT_COLUMNS;
use super::get_project_by_id;
use crate::database::{
    database_operation_error, format_timestamp, validate_nonblank_value, validate_usage_percentage,
};
use crate::mapping::map_chat_row;
use crate::{
    Chat, ChatId, ChatInitializationParameters, ChatUpdateOptions, InfrastructureError,
    InfrastructureErrorKind, InfrastructureResult, ProjectId,
};
use sqlx::{AssertSqlSafe, Sqlite, Transaction};
use time::OffsetDateTime;

pub(crate) async fn create_chat(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_name: String,
    project_id: ProjectId,
    initialization_parameters: ChatInitializationParameters,
) -> InfrastructureResult<Chat> {
    validate_nonblank_value("chat name", &chat_name)?;
    validate_usage_percentage(
        "chat CPU usage percentage",
        initialization_parameters.cpu_usage_percentage,
    )?;
    validate_usage_percentage(
        "chat GPU usage percentage",
        initialization_parameters.gpu_usage_percentage,
    )?;

    if get_project_by_id(database_transaction, project_id)
        .await?
        .is_none()
    {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidEntityReference,
            format!("project '{project_id}' does not exist"),
        ));
    }

    let chat_id = ChatId::new();
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    sqlx::query(
        "INSERT INTO chats (id, project_id, name, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(chat_id.as_bytes().to_vec())
    .bind(project_id.as_bytes().to_vec())
    .bind(&chat_name)
    .bind(initialization_parameters.llm_thinking_enabled)
    .bind(initialization_parameters.llm_context_optimization_enabled)
    .bind(initialization_parameters.cpu_usage_percentage as i64)
    .bind(initialization_parameters.gpu_usage_percentage as i64)
    .bind(&created_at_text)
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to create chat", source))?;

    Ok(Chat {
        id: chat_id,
        project_id,
        name: chat_name,
        llm_thinking_enabled: initialization_parameters.llm_thinking_enabled,
        llm_context_optimization_enabled: initialization_parameters
            .llm_context_optimization_enabled,
        cpu_usage_percentage: initialization_parameters.cpu_usage_percentage,
        gpu_usage_percentage: initialization_parameters.gpu_usage_percentage,
        created_at,
        updated_at: created_at,
    })
}

pub(crate) async fn get_chat_by_id(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
) -> InfrastructureResult<Option<Chat>> {
    let query_text = format!("SELECT {CHAT_COLUMNS} FROM chats WHERE id = ?");
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(chat_id.as_bytes().to_vec())
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get chat by ID", source))?;
    database_row.as_ref().map(map_chat_row).transpose()
}

pub(crate) async fn update_chat(
    database_transaction: &mut Transaction<'static, Sqlite>,
    update_options: ChatUpdateOptions,
) -> InfrastructureResult<Option<Chat>> {
    let Some(existing_chat) = get_chat_by_id(database_transaction, update_options.chat_id).await?
    else {
        return Ok(None);
    };

    if !update_options.contains_changes() {
        return Ok(Some(existing_chat));
    }

    let chat_name = update_options.name.unwrap_or(existing_chat.name);
    let llm_thinking_enabled = update_options
        .llm_thinking_enabled
        .unwrap_or(existing_chat.llm_thinking_enabled);
    let llm_context_optimization_enabled = update_options
        .llm_context_optimization_enabled
        .unwrap_or(existing_chat.llm_context_optimization_enabled);
    let cpu_usage_percentage = update_options
        .cpu_usage_percentage
        .unwrap_or(existing_chat.cpu_usage_percentage);
    let gpu_usage_percentage = update_options
        .gpu_usage_percentage
        .unwrap_or(existing_chat.gpu_usage_percentage);
    validate_nonblank_value("chat name", &chat_name)?;
    validate_usage_percentage("chat CPU usage percentage", cpu_usage_percentage)?;
    validate_usage_percentage("chat GPU usage percentage", gpu_usage_percentage)?;
    let updated_at = OffsetDateTime::now_utc();
    let updated_at_text = format_timestamp(updated_at)?;

    sqlx::query(
        "UPDATE chats SET name = ?, llm_thinking_enabled = ?, llm_context_optimization_enabled = ?, cpu_usage_percentage = ?, gpu_usage_percentage = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&chat_name)
    .bind(llm_thinking_enabled)
    .bind(llm_context_optimization_enabled)
    .bind(cpu_usage_percentage as i64)
    .bind(gpu_usage_percentage as i64)
    .bind(&updated_at_text)
    .bind(update_options.chat_id.as_bytes().to_vec())
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to update chat", source))?;

    Ok(Some(Chat {
        id: update_options.chat_id,
        project_id: existing_chat.project_id,
        name: chat_name,
        llm_thinking_enabled,
        llm_context_optimization_enabled,
        cpu_usage_percentage,
        gpu_usage_percentage,
        created_at: existing_chat.created_at,
        updated_at,
    }))
}

pub(crate) async fn delete_chat_by_id(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
) -> InfrastructureResult<Option<Chat>> {
    let Some(chat) = get_chat_by_id(database_transaction, chat_id).await? else {
        return Ok(None);
    };
    sqlx::query("DELETE FROM chats WHERE id = ?")
        .bind(chat_id.as_bytes().to_vec())
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to delete chat", source))?;

    Ok(Some(chat))
}
