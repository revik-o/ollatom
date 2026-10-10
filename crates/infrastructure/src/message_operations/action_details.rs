use super::*;

pub(crate) async fn insert_llm_action(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    summary: Option<String>,
    details: LlmActionDetails,
    initial_status_event_input: LlmActionStatusEventInput,
    status_event_inputs: Vec<LlmActionStatusEventInput>,
) -> InfrastructureResult<LlmAction> {
    details.validate().map_err(|message| {
        InfrastructureError::new(InfrastructureErrorKind::EntityValidationFailed, message)
    })?;
    let mut status_sequence = Vec::new();

    status_sequence.push(initial_status_event_input.clone());
    status_sequence.extend(status_event_inputs.iter().cloned());
    validate_action_status_event_sequence(&status_sequence)?;

    let sequence_number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM llm_actions WHERE message_id = ?",
    )
    .bind(message_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to allocate LLM action sequence", source))?;

    let llm_action_id = LlmActionId::new();
    let sequence_number = u32::try_from(sequence_number).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::CorruptDatabaseValue,
            format!("allocated LLM action sequence is outside the u32 range: {source}"),
        )
    })?;

    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    let initial_status_occurred_at = OffsetDateTime::now_utc();
    let initial_status_occurred_at_text = format_timestamp(initial_status_occurred_at)?;
    let initial_status_payload_json = initial_status_event_input
        .payload
        .as_ref()
        .map(serialize_json)
        .transpose()?;

    let action_details_json = serialize_action_details(&details)?;

    sqlx::query(
        "INSERT INTO llm_actions (id, message_id, sequence_number, action_kind, action_details_json, summary, initial_status, initial_status_payload_json, initial_status_occurred_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(message_id.as_bytes().to_vec())
    .bind(i64::from(sequence_number))
    .bind(action_kind_text(&details))
    .bind(action_details_json)
    .bind(&summary)
    .bind(action_status_text(initial_status_event_input.status))
    .bind(initial_status_payload_json)
    .bind(&initial_status_occurred_at_text)
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert LLM action", source))?;

    let mut status_events = Vec::with_capacity(status_event_inputs.len());

    for (position, status_event_input) in status_event_inputs.into_iter().enumerate() {
        let sequence_number = u32::try_from(position)
            .ok()
            .and_then(|position| position.checked_add(2))
            .ok_or_else(|| {
                InfrastructureError::new(
                    InfrastructureErrorKind::EntityValidationFailed,
                    "LLM action status event sequence exceeds the u32 range",
                )
            })?;
        status_events.push(
            insert_action_status_event(
                database_transaction,
                llm_action_id,
                sequence_number,
                status_event_input,
            )
            .await?,
        );
    }

    Ok(LlmAction {
        id: llm_action_id,
        message_id,
        sequence_number,
        summary,
        details,
        initial_status: LlmActionInitialStatus {
            status: initial_status_event_input.status,
            payload: initial_status_event_input.payload,
            occurred_at: initial_status_occurred_at,
        },
        status_events,
        created_at,
    })
}

fn serialize_action_details(action_details: &LlmActionDetails) -> InfrastructureResult<String> {
    let action_details_value = match action_details {
        LlmActionDetails::FileChange(details) => serde_json::json!({
            "kind": "file_change",
            "operation": file_change_operation_text(details.operation),
            "source_path": &details.source_path,
            "destination_path": &details.destination_path,
            "content_before": &details.content_before,
            "content_after": &details.content_after,
            "unified_diff": &details.unified_diff,
            "metadata": &details.metadata,
        }),
        LlmActionDetails::Command(details) => serde_json::json!({
            "kind": "command",
            "command_text": &details.command_text,
            "working_directory": &details.working_directory,
            "environment": &details.environment,
        }),
        LlmActionDetails::ToolCall(details) => serde_json::json!({
            "kind": "tool_call",
            "tool_name": &details.tool_name,
            "arguments": &details.arguments,
        }),
    };

    serialize_json(&action_details_value)
}
