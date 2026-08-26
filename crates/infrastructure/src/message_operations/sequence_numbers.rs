use super::*;

pub(crate) async fn allocate_next_active_message_sequence_number(
    database_transaction: &mut Transaction<'static, Sqlite>,
    chat_id: ChatId,
) -> InfrastructureResult<u64> {
    let sequence_number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM messages WHERE chat_id = ? AND validity = 'active'",
    )
    .bind(chat_id.as_bytes().to_vec())
    .fetch_one(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to allocate message sequence", source))?;

    Ok(sequence_number as u64)
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

    Ok(response_round_number as u32)
}
