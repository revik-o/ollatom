use super::*;

pub(crate) async fn add_message_from_llm(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_contents: String,
    action_inputs: Vec<LlmActionInput>,
    chat_id: ChatId,
    user_message_id: MessageId,
) -> InfrastructureResult<Message> {
    validate_llm_reply_target(database_transaction, chat_id, user_message_id).await?;

    if message_contents.trim().is_empty() && action_inputs.is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            "completed LLM message must contain text or at least one action",
        ));
    }

    for action_input in &action_inputs {
        validate_batch_action_status_events(&action_input.status_events)?;
    }

    let message = insert_llm_message(
        database_transaction,
        chat_id,
        user_message_id,
        message_contents,
        LlmMessageState::Completed,
    )
    .await?;

    for action_input in action_inputs {
        insert_llm_action(
            database_transaction,
            message.id,
            action_input.summary,
            action_input.details,
            action_input.status_events,
        )
        .await?;
    }

    Ok(message)
}

pub(crate) async fn begin_llm_message(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
    user_message_id: MessageId,
) -> InfrastructureResult<Message> {
    validate_llm_reply_target(database_transaction, chat_id, user_message_id).await?;
    insert_llm_message(
        database_transaction,
        chat_id,
        user_message_id,
        String::new(),
        LlmMessageState::InProgress,
    )
    .await
}

pub(crate) async fn complete_llm_message(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    completed_message_contents: String,
) -> InfrastructureResult<Option<Message>> {
    let Some(message) = load_message_by_id(database_transaction, message_id).await? else {
        return Ok(None);
    };
    validate_in_progress_llm_message(&message)?;
    let action_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM llm_actions WHERE message_id = ?")
            .bind(message_id.as_bytes().to_vec())
            .fetch_one(&mut **database_transaction)
            .await
            .map_err(|source| database_operation_error("failed to count LLM actions", source))?;
    if completed_message_contents.trim().is_empty() && action_count == 0 {
        return Err(invalid_message_operation(
            "completed LLM message must contain text or at least one action",
        ));
    }
    ensure_all_actions_are_terminal(database_transaction, message_id).await?;
    update_llm_message_state(
        database_transaction,
        message_id,
        LlmMessageState::Completed,
        completed_message_contents,
    )
    .await?;
    load_message_by_id(database_transaction, message_id).await
}

pub(crate) async fn finish_llm_message_unsuccessfully(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    final_state: LlmMessageState,
    final_message_contents: String,
) -> InfrastructureResult<Option<Message>> {
    let Some(message) = load_message_by_id(database_transaction, message_id).await? else {
        return Ok(None);
    };
    validate_in_progress_llm_message(&message)?;
    cancel_unfinished_actions(database_transaction, message_id).await?;
    update_llm_message_state(
        database_transaction,
        message_id,
        final_state,
        final_message_contents,
    )
    .await?;
    load_message_by_id(database_transaction, message_id).await
}

pub(crate) async fn insert_llm_message(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
    user_message_id: MessageId,
    message_contents: String,
    llm_message_state: LlmMessageState,
) -> InfrastructureResult<Message> {
    let message_id = MessageId::new();
    let sequence_number =
        allocate_next_active_message_sequence_number(database_transaction, chat_id).await?;
    let response_round_number =
        allocate_next_llm_response_round_number(database_transaction, user_message_id).await?;
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    sqlx::query(
        "INSERT INTO messages (id, chat_id, sequence_number, role, contents, user_revision_group_id, user_revision_number, llm_reply_to_user_message_id, llm_response_round_number, llm_message_state, validity, created_at, updated_at, deprecated_at) VALUES (?, ?, ?, 'llm', ?, NULL, NULL, ?, ?, ?, 'active', ?, NULL, NULL)",
    )
    .bind(message_id.as_bytes().to_vec())
    .bind(chat_id.as_bytes().to_vec())
    .bind(sequence_number as i64)
    .bind(&message_contents)
    .bind(user_message_id.as_bytes().to_vec())
    .bind(response_round_number as i64)
    .bind(llm_message_state_text(llm_message_state))
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to add LLM message", source))?;

    Ok(Message {
        id: message_id,
        chat_id,
        sequence_number,
        contents: message_contents,
        attachments: Vec::new(),
        role_metadata: MessageRoleMetadata::Llm(crate::LlmMessageMetadata {
            llm_reply_to_user_message_id: user_message_id,
            llm_response_round_number: response_round_number,
            llm_message_state,
        }),
        validity: MessageValidity::Active,
        created_at,
        updated_at: None,
        deprecated_at: None,
    })
}

pub(crate) async fn update_llm_message_state(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    final_state: LlmMessageState,
    final_message_contents: String,
) -> InfrastructureResult<()> {
    let updated_at_text = format_timestamp(OffsetDateTime::now_utc())?;
    sqlx::query(
        "UPDATE messages SET contents = ?, llm_message_state = ?, updated_at = ? WHERE id = ? AND role = 'llm'",
    )
    .bind(final_message_contents)
    .bind(llm_message_state_text(final_state))
    .bind(updated_at_text)
    .bind(message_id.as_bytes().to_vec())
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to update LLM message state", source))?;

    Ok(())
}
