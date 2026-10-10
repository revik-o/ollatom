use super::*;

#[tokio::test]
async fn action_journal_preserves_initial_status_and_rejects_mutation() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let (user_message_id, llm_message_id, action_id) =
        create_pending_command_action(&infrastructure, &chat).await;

    let action_update_result =
        sqlx::query("UPDATE llm_actions SET summary = 'Rewritten' WHERE id = ?")
            .bind(action_id.as_bytes().to_vec())
            .execute(&infrastructure.connection_pool)
            .await;
    assert!(action_update_result.is_err());

    let illegal_initial_transition = sqlx::query(
        "INSERT INTO action_status_events (id, llm_action_id, sequence_number, status, payload_json, occurred_at) VALUES (?, ?, 2, 'succeeded', NULL, '2026-09-26T00:00:00Z')",
    )
    .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
    .bind(action_id.as_bytes().to_vec())
    .execute(&infrastructure.connection_pool)
    .await;
    assert!(illegal_initial_transition.is_err());

    let sequence_gap = sqlx::query(
        "INSERT INTO action_status_events (id, llm_action_id, sequence_number, status, payload_json, occurred_at) VALUES (?, ?, 3, 'running', NULL, '2026-09-26T00:00:00Z')",
    )
    .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
    .bind(action_id.as_bytes().to_vec())
    .execute(&infrastructure.connection_pool)
    .await;
    assert!(sequence_gap.is_err());

    let action_for_user_message = insert_raw_action(
        &infrastructure,
        user_message_id,
        1,
        "command",
        r#"{"kind":"command","command_text":"echo result","working_directory":"/tmp","environment":{}}"#,
    )
    .await;
    assert!(action_for_user_message.is_err());

    let running_event = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .append_llm_action_status_event(
                    action_id,
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Running,
                        payload: None,
                    },
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(running_event.sequence_number, 2);

    let status_update_result =
        sqlx::query("UPDATE action_status_events SET status = 'failed' WHERE id = ?")
            .bind(running_event.id.as_bytes().to_vec())
            .execute(&infrastructure.connection_pool)
            .await;
    assert!(status_update_result.is_err());

    let status_delete_result = sqlx::query("DELETE FROM action_status_events WHERE id = ?")
        .bind(running_event.id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await;
    assert!(status_delete_result.is_err());

    let action_delete_result = sqlx::query("DELETE FROM llm_actions WHERE id = ?")
        .bind(action_id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await;
    assert!(action_delete_result.is_err());

    let illegal_running_transition = sqlx::query(
        "INSERT INTO action_status_events (id, llm_action_id, sequence_number, status, payload_json, occurred_at) VALUES (?, ?, 3, 'pending', NULL, '2026-09-26T00:00:00Z')",
    )
    .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
    .bind(action_id.as_bytes().to_vec())
    .execute(&infrastructure.connection_pool)
    .await;
    assert!(illegal_running_transition.is_err());

    let succeeded_event = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .append_llm_action_status_event(
                    action_id,
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Succeeded,
                        payload: None,
                    },
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(succeeded_event.sequence_number, 3);

    let completion_result =
        sqlx::query("UPDATE messages SET llm_message_state = 'completed' WHERE id = ?")
            .bind(llm_message_id.as_bytes().to_vec())
            .execute(&infrastructure.connection_pool)
            .await;
    assert!(completion_result.is_ok());
}
