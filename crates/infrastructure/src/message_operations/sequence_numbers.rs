use super::*;

pub(crate) async fn allocate_next_active_message_sequence_number(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
) -> InfrastructureResult<u32> {
    let sequence_number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM messages WHERE chat_id = ? AND validity = 'active'",
    )
    .bind(chat_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to allocate message sequence", source))?;

    u32::try_from(sequence_number).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::CorruptDatabaseValue,
            format!("allocated message sequence is outside the u32 range: {source}"),
        )
    })
}

pub(crate) async fn allocate_next_llm_response_round_number(
    database_transaction: &mut Transaction<'static, Sqlite>,
    user_message_id: MessageId,
) -> InfrastructureResult<u32> {
    let response_round_number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(llm_response_round_number), 0) + 1 FROM messages WHERE llm_reply_to_user_message_id = ? AND role = 'llm'",
    )
    .bind(user_message_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to allocate LLM response round", source))?;

    u32::try_from(response_round_number).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::CorruptDatabaseValue,
            format!("allocated LLM response round is outside the u32 range: {source}"),
        )
    })
}
