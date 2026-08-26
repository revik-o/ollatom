use super::*;

pub(crate) async fn insert_llm_action(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    summary: Option<String>,
    details: LlmActionDetails,
    status_event_inputs: Vec<LlmActionStatusEventInput>,
) -> InfrastructureResult<LlmAction> {
    if status_event_inputs.is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidLlmActionStatusTransition,
            "LLM action must contain at least one status event",
        ));
    }
    validate_action_status_event_sequence(&status_event_inputs)?;
    let sequence_number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM llm_actions WHERE message_id = ?",
    )
    .bind(message_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to allocate LLM action sequence", source))?;
    let llm_action_id = LlmActionId::new();
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    sqlx::query(
        "INSERT INTO llm_actions (id, message_id, sequence_number, action_kind, summary, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(message_id.as_bytes().to_vec())
    .bind(sequence_number)
    .bind(action_kind_text(&details))
    .bind(&summary)
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert LLM action", source))?;
    insert_action_details(database_transaction, llm_action_id, &details).await?;

    let mut status_events = Vec::with_capacity(status_event_inputs.len());
    for (position, status_event_input) in status_event_inputs.into_iter().enumerate() {
        status_events.push(
            insert_action_status_event(
                database_transaction,
                llm_action_id,
                position as u32 + 1,
                status_event_input,
            )
            .await?,
        );
    }

    Ok(LlmAction {
        id: llm_action_id,
        message_id,
        sequence_number: sequence_number as u32,
        summary,
        details,
        status_events,
        created_at,
    })
}

pub(crate) async fn insert_action_details(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    details: &LlmActionDetails,
) -> InfrastructureResult<()> {
    match details {
        LlmActionDetails::FileChange(file_change_details) => {
            insert_file_change_details(database_transaction, llm_action_id, file_change_details)
                .await
        }
        LlmActionDetails::Command(command_action_details) => {
            insert_command_action_details(
                database_transaction,
                llm_action_id,
                command_action_details,
            )
            .await
        }
        LlmActionDetails::ToolCall(tool_call_action_details) => {
            insert_tool_call_action_details(
                database_transaction,
                llm_action_id,
                tool_call_action_details,
            )
            .await
        }
    }
}

pub(crate) async fn insert_file_change_details(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    details: &FileChangeDetails,
) -> InfrastructureResult<()> {
    let metadata_json = serialize_json(&details.metadata)?;
    sqlx::query(
        "INSERT INTO file_action_details (llm_action_id, operation, source_path, destination_path, content_before, content_after, unified_diff, metadata_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(file_change_operation_text(details.operation))
    .bind(&details.source_path)
    .bind(&details.destination_path)
    .bind(&details.content_before)
    .bind(&details.content_after)
    .bind(&details.unified_diff)
    .bind(metadata_json)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert file action details", source))?;
    Ok(())
}

pub(crate) async fn insert_command_action_details(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    details: &CommandActionDetails,
) -> InfrastructureResult<()> {
    let environment_json = serde_json::to_string(&details.environment).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to serialize command environment: {source}"),
        )
    })?;
    sqlx::query(
        "INSERT INTO command_action_details (llm_action_id, command_text, working_directory, environment_json) VALUES (?, ?, ?, ?)",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(&details.command_text)
    .bind(&details.working_directory)
    .bind(environment_json)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert command action details", source))?;
    Ok(())
}

pub(crate) async fn insert_tool_call_action_details(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    details: &ToolCallActionDetails,
) -> InfrastructureResult<()> {
    let arguments_json = serialize_json(&details.arguments)?;
    sqlx::query(
        "INSERT INTO tool_call_action_details (llm_action_id, tool_name, arguments_json) VALUES (?, ?, ?)",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(&details.tool_name)
    .bind(arguments_json)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert tool action details", source))?;
    Ok(())
}
