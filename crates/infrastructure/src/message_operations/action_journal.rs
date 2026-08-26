use super::*;

pub(crate) async fn add_llm_action_to_message(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    summary: Option<String>,
    action_details: LlmActionDetails,
    initial_status_event: LlmActionStatusEventInput,
) -> InfrastructureResult<LlmAction> {
    let Some(message) = load_message_by_id(database_transaction, message_id).await? else {
        return Err(invalid_message_operation("LLM message does not exist"));
    };
    let MessageRoleMetadata::Llm(llm_message_metadata) = message.role_metadata else {
        return Err(invalid_message_operation(
            "actions can only be added to an LLM message",
        ));
    };
    if llm_message_metadata.llm_message_state != LlmMessageState::InProgress
        || message.validity != MessageValidity::Active
    {
        return Err(invalid_message_operation(
            "actions can only be added to an active in-progress LLM message",
        ));
    }

    insert_llm_action(
        database_transaction,
        message_id,
        summary,
        action_details,
        vec![initial_status_event],
    )
    .await
}

pub(crate) async fn append_llm_action_status_event(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    status_event_input: LlmActionStatusEventInput,
) -> InfrastructureResult<LlmActionStatusEvent> {
    let latest_event_row = sqlx::query(
        "SELECT sequence_number, status FROM action_status_events WHERE llm_action_id = ? ORDER BY sequence_number DESC LIMIT 1",
    )
    .bind(llm_action_id.as_bytes().to_vec())
    .fetch_optional(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to read latest action status", source))?;

    let Some(latest_event_row) = latest_event_row else {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidEntityReference,
            format!("LLM action '{llm_action_id}' does not exist"),
        ));
    };
    let latest_sequence_number: i64 = latest_event_row
        .try_get("sequence_number")
        .map_err(|source| database_operation_error("failed to decode action sequence", source))?;
    let latest_status_text: String = latest_event_row
        .try_get("status")
        .map_err(|source| database_operation_error("failed to decode action status", source))?;
    let latest_status = parse_action_status(&latest_status_text)?;
    validate_action_status_transition(latest_status, status_event_input.status)?;
    insert_action_status_event(
        database_transaction,
        llm_action_id,
        (latest_sequence_number + 1) as u32,
        status_event_input,
    )
    .await
}

pub(crate) async fn insert_action_status_event(
    database_transaction: &mut Transaction<'static, Sqlite>,
    llm_action_id: LlmActionId,
    sequence_number: u32,
    status_event_input: LlmActionStatusEventInput,
) -> InfrastructureResult<LlmActionStatusEvent> {
    let status_event_id = LlmActionStatusEventId::new();
    let occurred_at = OffsetDateTime::now_utc();
    let occurred_at_text = format_timestamp(occurred_at)?;
    let payload_json = status_event_input
        .payload
        .as_ref()
        .map(serialize_json)
        .transpose()?;
    sqlx::query(
        "INSERT INTO action_status_events (id, llm_action_id, sequence_number, status, payload_json, occurred_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(status_event_id.as_bytes().to_vec())
    .bind(llm_action_id.as_bytes().to_vec())
    .bind(sequence_number as i64)
    .bind(action_status_text(status_event_input.status))
    .bind(payload_json)
    .bind(&occurred_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to append action status event", source))?;

    Ok(LlmActionStatusEvent {
        id: status_event_id,
        llm_action_id,
        sequence_number,
        status: status_event_input.status,
        payload: status_event_input.payload,
        occurred_at,
    })
}

pub(crate) async fn ensure_all_actions_are_terminal(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
) -> InfrastructureResult<()> {
    let unfinished_action_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM llm_actions AS action WHERE action.message_id = ? AND NOT EXISTS (SELECT 1 FROM action_status_events AS status_event WHERE status_event.llm_action_id = action.id AND status_event.sequence_number = (SELECT MAX(latest_status_event.sequence_number) FROM action_status_events AS latest_status_event WHERE latest_status_event.llm_action_id = action.id) AND status_event.status IN ('succeeded', 'failed', 'cancelled'))",
    )
    .bind(message_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to validate LLM action states", source))?;

    if unfinished_action_count > 0 {
        return Err(invalid_message_operation(
            "LLM message cannot finish while actions are unfinished",
        ));
    }

    Ok(())
}

pub(crate) async fn cancel_unfinished_actions(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
) -> InfrastructureResult<()> {
    let action_rows = sqlx::query(
        "SELECT action.id, status_event.status FROM llm_actions AS action JOIN action_status_events AS status_event ON status_event.llm_action_id = action.id WHERE action.message_id = ? AND status_event.sequence_number = (SELECT MAX(latest_status_event.sequence_number) FROM action_status_events AS latest_status_event WHERE latest_status_event.llm_action_id = action.id)",
    )
    .bind(message_id.as_bytes().to_vec())
    .fetch_all(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to read unfinished actions", source))?;

    for action_row in action_rows {
        let status_text: String = action_row
            .try_get("status")
            .map_err(|source| database_operation_error("failed to decode action status", source))?;
        let status = parse_action_status(&status_text)?;
        if !status.is_terminal() {
            let action_identifier_bytes: Vec<u8> = action_row
                .try_get("id")
                .map_err(|source| database_operation_error("failed to decode action ID", source))?;
            let action_id =
                LlmActionId::from(crate::database::parse_uuid(action_identifier_bytes)?);
            append_llm_action_status_event(
                database_transaction,
                action_id,
                LlmActionStatusEventInput {
                    status: LlmActionStatus::Cancelled,
                    payload: None,
                },
            )
            .await?;
        }
    }

    Ok(())
}
