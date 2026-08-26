use super::*;

pub(crate) async fn validate_chat_exists(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
) -> InfrastructureResult<()> {
    let chat_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chats WHERE id = ?")
        .bind(chat_id.as_bytes().to_vec())
        .fetch_one(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to validate chat", source))?;

    if chat_exists == 0 {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidEntityReference,
            format!("chat '{chat_id}' does not exist"),
        ));
    }

    Ok(())
}

pub(crate) async fn validate_llm_reply_target(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
    user_message_id: MessageId,
) -> InfrastructureResult<()> {
    validate_chat_exists(database_transaction, chat_id).await?;
    let Some(user_message) = load_message_by_id(database_transaction, user_message_id).await?
    else {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidEntityReference,
            format!("user message '{user_message_id}' does not exist"),
        ));
    };

    if user_message.chat_id != chat_id
        || user_message.validity != MessageValidity::Active
        || !matches!(user_message.role_metadata, MessageRoleMetadata::User(_))
    {
        return Err(invalid_message_operation(
            "LLM reply target must be an active user message in the same chat",
        ));
    }

    Ok(())
}

pub(crate) fn validate_message_contents_and_attachments(
    message_contents: &str,
    attachment_inputs: &[AttachmentInput],
) -> InfrastructureResult<()> {
    if message_contents.trim().is_empty() && attachment_inputs.is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            "user message must contain text or at least one attachment",
        ));
    }

    Ok(())
}

pub(crate) fn validate_attachment_inputs(
    attachment_inputs: &[AttachmentInput],
) -> InfrastructureResult<()> {
    for attachment_input in attachment_inputs {
        validate_nonblank_value("attachment file name", &attachment_input.file_name)?;
        validate_nonblank_value(
            "attachment storage reference",
            &attachment_input.storage_reference,
        )?;
        i64::try_from(attachment_input.byte_length).map_err(|source| {
            InfrastructureError::new(
                InfrastructureErrorKind::EntityValidationFailed,
                format!("attachment byte length exceeds SQLite range: {source}"),
            )
        })?;
    }

    Ok(())
}

pub(crate) fn validate_batch_action_status_events(
    status_events: &[LlmActionStatusEventInput],
) -> InfrastructureResult<()> {
    validate_action_status_event_sequence(status_events)?;

    if !status_events
        .last()
        .is_some_and(|status_event| status_event.status.is_terminal())
    {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidLlmActionStatusTransition,
            "batched LLM action must finish with a terminal status",
        ));
    }

    Ok(())
}

pub(crate) fn validate_action_status_event_sequence(
    status_events: &[LlmActionStatusEventInput],
) -> InfrastructureResult<()> {
    if status_events.is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidLlmActionStatusTransition,
            "LLM action requires at least one status event",
        ));
    }

    for status_event_pair in status_events.windows(2) {
        validate_action_status_transition(
            status_event_pair[0].status,
            status_event_pair[1].status,
        )?;
    }

    Ok(())
}

pub(crate) fn validate_action_status_transition(
    previous_status: LlmActionStatus,
    next_status: LlmActionStatus,
) -> InfrastructureResult<()> {
    let is_valid_transition = match previous_status {
        LlmActionStatus::Pending => matches!(
            next_status,
            LlmActionStatus::Running
                | LlmActionStatus::Succeeded
                | LlmActionStatus::Failed
                | LlmActionStatus::Cancelled
        ),
        LlmActionStatus::Running => matches!(
            next_status,
            LlmActionStatus::Succeeded | LlmActionStatus::Failed | LlmActionStatus::Cancelled
        ),
        LlmActionStatus::Succeeded | LlmActionStatus::Failed | LlmActionStatus::Cancelled => false,
    };

    if !is_valid_transition {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidLlmActionStatusTransition,
            format!(
                "invalid LLM action status transition from {previous_status:?} to {next_status:?}"
            ),
        ));
    }

    Ok(())
}

pub(crate) fn validate_in_progress_llm_message(message: &Message) -> InfrastructureResult<()> {
    let MessageRoleMetadata::Llm(llm_message_metadata) = &message.role_metadata else {
        return Err(invalid_message_operation("message is not an LLM message"));
    };

    if message.validity != MessageValidity::Active
        || llm_message_metadata.llm_message_state != LlmMessageState::InProgress
    {
        return Err(invalid_message_operation(
            "LLM message is not active and in progress",
        ));
    }

    Ok(())
}

pub(crate) fn action_kind_text(details: &LlmActionDetails) -> &'static str {
    match details {
        LlmActionDetails::FileChange(_) => "file_change",
        LlmActionDetails::Command(_) => "command",
        LlmActionDetails::ToolCall(_) => "tool_call",
    }
}

pub(crate) fn file_change_operation_text(operation: FileChangeOperation) -> &'static str {
    match operation {
        FileChangeOperation::Create => "create",
        FileChangeOperation::Modify => "modify",
        FileChangeOperation::Delete => "delete",
        FileChangeOperation::Rename => "rename",
    }
}

pub(crate) fn action_status_text(status: LlmActionStatus) -> &'static str {
    match status {
        LlmActionStatus::Pending => "pending",
        LlmActionStatus::Running => "running",
        LlmActionStatus::Succeeded => "succeeded",
        LlmActionStatus::Failed => "failed",
        LlmActionStatus::Cancelled => "cancelled",
    }
}

pub(crate) fn parse_action_status(status: &str) -> InfrastructureResult<LlmActionStatus> {
    match status {
        "pending" => Ok(LlmActionStatus::Pending),
        "running" => Ok(LlmActionStatus::Running),
        "succeeded" => Ok(LlmActionStatus::Succeeded),
        "failed" => Ok(LlmActionStatus::Failed),
        "cancelled" => Ok(LlmActionStatus::Cancelled),
        unexpected_status => Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored LLM action status '{unexpected_status}' is invalid"),
        )),
    }
}

pub(crate) fn llm_message_state_text(state: LlmMessageState) -> &'static str {
    match state {
        LlmMessageState::InProgress => "in_progress",
        LlmMessageState::Completed => "completed",
        LlmMessageState::Failed => "failed",
        LlmMessageState::Cancelled => "cancelled",
    }
}

pub(crate) fn invalid_message_operation(message: impl Into<String>) -> InfrastructureError {
    InfrastructureError::new(InfrastructureErrorKind::InvalidMessageOperation, message)
}
