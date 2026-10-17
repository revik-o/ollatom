use super::*;

#[tokio::test]
async fn action_details_are_validated_before_insert_and_parent_completion() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let invalid_details = vec![
        LlmActionDetails::FileChange(FileChangeDetails {
            operation: FileChangeOperation::Create,
            source_path: " ".to_owned(),
            destination_path: None,
            content_before: None,
            content_after: None,
            unified_diff: None,
            metadata: serde_json::json!({}),
        }),
        LlmActionDetails::FileChange(FileChangeDetails {
            operation: FileChangeOperation::Rename,
            source_path: "source.txt".to_owned(),
            destination_path: None,
            content_before: None,
            content_after: None,
            unified_diff: None,
            metadata: serde_json::json!({}),
        }),
        LlmActionDetails::Command(CommandActionDetails {
            command_text: " ".to_owned(),
            working_directory: "/tmp".to_owned(),
            environment: BTreeMap::new(),
        }),
        LlmActionDetails::Command(CommandActionDetails {
            command_text: "echo result".to_owned(),
            working_directory: " ".to_owned(),
            environment: BTreeMap::new(),
        }),
        LlmActionDetails::ToolCall(crate::ToolCallActionDetails {
            tool_name: " ".to_owned(),
            arguments: serde_json::json!({}),
        }),
    ];

    for action_details in invalid_details {
        let validation_error = infrastructure
            .execute_db_actions(async |transaction| {
                let user_message = transaction.add_message_from_user("Question", Vec::new(), &chat).await?;
                let llm_message = transaction
                    .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                    .await?;
                transaction
                    .add_llm_action_to_message_by_id(
                        llm_message.id,
                        None,
                        action_details,
                        LlmActionStatusEventInput {
                            status: LlmActionStatus::Pending,
                            payload: None,
                        },
                    )
                    .await
            })
            .await
            .unwrap_err();

        assert_eq!(validation_error.kind(), InfrastructureErrorKind::EntityValidationFailed);
    }

    let (_, llm_message_id, action_id) = create_pending_command_action(&infrastructure, &chat).await;
    let invalid_database_details = [
        (
            "file_change",
            r#"{"kind":"file_change","operation":"rename","source_path":"source.txt","destination_path":null,"content_before":null,"content_after":null,"unified_diff":null,"metadata":{}}"#,
        ),
        (
            "file_change",
            r#"{"kind":"file_change","operation":"create","source_path":" ","destination_path":null,"content_before":null,"content_after":null,"unified_diff":null,"metadata":{}}"#,
        ),
        (
            "command",
            r#"{"kind":"command","command_text":" ","working_directory":"/tmp","environment":{}}"#,
        ),
        (
            "tool_call",
            r#"{"kind":"tool_call","tool_name":"search","arguments":[]}"#,
        ),
        ("command", r#"{"kind":"tool_call","tool_name":"search","arguments":{}}"#),
    ];
    for (action_kind, action_details_json) in invalid_database_details {
        assert!(
            insert_raw_action(&infrastructure, llm_message_id, 2, action_kind, action_details_json,)
                .await
                .is_err()
        );
    }

    let incomplete_message_completion = sqlx::query("UPDATE messages SET llm_message_state = 'completed' WHERE id = ?")
        .bind(llm_message_id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await;
    assert!(incomplete_message_completion.is_err());

    infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .append_llm_action_status_event(
                    action_id,
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Running,
                        payload: None,
                    },
                )
                .await?;
            transaction
                .append_llm_action_status_event(
                    action_id,
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Succeeded,
                        payload: None,
                    },
                )
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    let completion_result = sqlx::query("UPDATE messages SET llm_message_state = 'completed' WHERE id = ?")
        .bind(llm_message_id.as_bytes().to_vec())
        .execute(&infrastructure.connection_pool)
        .await;
    assert!(completion_result.is_ok());
}

#[tokio::test]
async fn database_rejects_incomplete_actions_on_insert() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let (user_message_id, llm_message_id, _) = create_pending_command_action(&infrastructure, &chat).await;
    let incomplete_action_id = uuid::Uuid::new_v4().as_bytes().to_vec();

    let incomplete_action_insert = sqlx::query(
        "INSERT INTO llm_actions (id, message_id, sequence_number, action_kind, summary, initial_status, initial_status_occurred_at, created_at) VALUES (?, ?, 2, 'command', NULL, 'pending', '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
    )
    .bind(&incomplete_action_id)
    .bind(llm_message_id.as_bytes().to_vec())
    .execute(&infrastructure.connection_pool)
    .await;
    assert!(incomplete_action_insert.is_err());

    let invalid_parent_action_insert = insert_raw_action(
        &infrastructure,
        user_message_id,
        1,
        "command",
        r#"{"kind":"command","command_text":"echo result","working_directory":"/tmp","environment":{}}"#,
    )
    .await;
    assert!(invalid_parent_action_insert.is_err());

    let incomplete_action_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_actions WHERE id = ?")
        .bind(&incomplete_action_id)
        .fetch_one(&infrastructure.connection_pool)
        .await
        .unwrap();
    assert_eq!(incomplete_action_count, 0);
}

#[tokio::test]
async fn deleting_parent_chat_cascades_through_the_action_aggregate() {
    let (_temporary_directory, infrastructure, chat) = create_test_chat().await;
    let (_, _, action_id) = create_pending_command_action(&infrastructure, &chat).await;

    infrastructure
        .execute_db_actions(async |transaction| transaction.delete_chat(chat.id).await)
        .await
        .unwrap();

    let remaining_action_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_actions WHERE id = ?")
        .bind(action_id.as_bytes().to_vec())
        .fetch_one(&infrastructure.connection_pool)
        .await
        .unwrap();
    assert_eq!(remaining_action_count, 0);
}
