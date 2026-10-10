use super::*;

#[tokio::test]
async fn message_sequence_allocation_rejects_values_above_u32() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let first_message = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("First message", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    sqlx::query("UPDATE messages SET sequence_number = ? WHERE id = ?")
        .bind(i64::from(u32::MAX))
        .bind(first_message.id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await
        .unwrap();

    let allocation_error = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Second message", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap_err();

    assert_eq!(
        allocation_error.kind(),
        InfrastructureErrorKind::CorruptDatabaseValue
    );
    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = ?")
        .bind(chat.id.as_bytes().to_vec())
        .fetch_one(&infrastructure.connection_pool)
        .await
        .unwrap();
    assert_eq!(message_count, 1);
}

#[tokio::test]
async fn response_round_allocation_rejects_values_above_u32() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let (user_message_id, llm_message_id, _) =
        create_pending_command_action(&infrastructure, &chat).await;
    sqlx::query("UPDATE messages SET llm_response_round_number = ? WHERE id = ?")
        .bind(i64::from(u32::MAX))
        .bind(llm_message_id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await
        .unwrap();

    let allocation_error = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message_id)
                .await
        })
        .await
        .unwrap_err();

    assert_eq!(
        allocation_error.kind(),
        InfrastructureErrorKind::CorruptDatabaseValue
    );
    let llm_message_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE llm_reply_to_user_message_id = ? AND role = 'llm'",
    )
    .bind(user_message_id.as_bytes().to_vec())
    .fetch_one(&infrastructure.connection_pool)
    .await
    .unwrap();
    assert_eq!(llm_message_count, 1);
}
