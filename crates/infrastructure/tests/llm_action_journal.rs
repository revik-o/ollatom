mod support;

use infrastructure::{
    CommandActionDetails, FileChangeDetails, FileChangeOperation, InfrastructureErrorKind,
    LlmActionDetails, LlmActionInput, LlmActionStatus, LlmActionStatusEventInput, LlmMessageState,
    MessageRoleMetadata, SqlValue, ToolCallActionDetails,
};
use serde_json::json;
use std::collections::BTreeMap;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

#[tokio::test]
async fn records_batched_command_action_status_history() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let llm_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Run tests", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm(
                    "Tests passed",
                    vec![LlmActionInput {
                        summary: Some("Run workspace tests".to_owned()),
                        details: LlmActionDetails::Command(CommandActionDetails {
                            command_text: "cargo test --workspace".to_owned(),
                            working_directory: "/projects/ollatom".to_owned(),
                            environment: BTreeMap::new(),
                        }),
                        status_events: vec![
                            LlmActionStatusEventInput {
                                status: LlmActionStatus::Running,
                                payload: None,
                            },
                            LlmActionStatusEventInput {
                                status: LlmActionStatus::Succeeded,
                                payload: Some(json!({
                                    "exit_code": 0,
                                    "standard_output": "ok",
                                    "standard_error": ""
                                })),
                            },
                        ],
                    }],
                    &user_message,
                    &chat,
                )
                .await
        })
        .await
        .unwrap();

    let action_event_count = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("COUNT(*) AS event_count")
        .from_as("action_status_events", "status_event")
        .inner_join_as("llm_actions", "action")
        .on("status_event.llm_action_id", "action.id")
        .filter(
            "action.message_id = {}",
            [SqlValue::from(llm_message.id.as_uuid())],
        )
        .commit()
        .await
        .unwrap()[0]
        .try_get::<i64>("event_count")
        .unwrap();
    assert_eq!(action_event_count, 2);
}

#[tokio::test]
async fn completes_live_llm_message_after_action_becomes_terminal() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let completed_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Run command", Vec::new(), &chat)
                .await?;
            let in_progress_message = transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                .await?;
            let action = transaction
                .add_llm_action_to_message_by_id(
                    in_progress_message.id,
                    Some("Run command".to_owned()),
                    LlmActionDetails::Command(CommandActionDetails {
                        command_text: "cargo check".to_owned(),
                        working_directory: "/projects/ollatom".to_owned(),
                        environment: BTreeMap::new(),
                    }),
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Running,
                        payload: None,
                    },
                )
                .await?;
            transaction
                .append_llm_action_status_event(
                    action.id,
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Succeeded,
                        payload: Some(json!({"exit_code": 0})),
                    },
                )
                .await?;
            transaction
                .complete_llm_message_by_id(in_progress_message.id, "Command succeeded")
                .await
        })
        .await
        .unwrap()
        .unwrap();

    let MessageRoleMetadata::Llm(llm_metadata) = completed_message.role_metadata else {
        panic!("expected LLM metadata");
    };
    assert_eq!(llm_metadata.llm_message_state, LlmMessageState::Completed);
}

#[tokio::test]
async fn persists_file_and_tool_action_details_and_json_payloads() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Apply changes", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm(
                    "Changes applied",
                    vec![
                        LlmActionInput {
                            summary: Some("Edit file".to_owned()),
                            details: LlmActionDetails::FileChange(FileChangeDetails {
                                operation: FileChangeOperation::Modify,
                                source_path: "src/main.rs".to_owned(),
                                destination_path: None,
                                content_before: Some("old".to_owned()),
                                content_after: Some("new".to_owned()),
                                unified_diff: Some("@@".to_owned()),
                                metadata: json!({"language": "rust"}),
                            }),
                            status_events: vec![LlmActionStatusEventInput {
                                status: LlmActionStatus::Succeeded,
                                payload: Some(json!({"file_result": "updated"})),
                            }],
                        },
                        LlmActionInput {
                            summary: Some("Call tool".to_owned()),
                            details: LlmActionDetails::ToolCall(ToolCallActionDetails {
                                tool_name: "formatter".to_owned(),
                                arguments: json!({"path": "src/main.rs"}),
                            }),
                            status_events: vec![LlmActionStatusEventInput {
                                status: LlmActionStatus::Failed,
                                payload: Some(json!({"failure": "tool unavailable"})),
                            }],
                        },
                    ],
                    &user_message,
                    &chat,
                )
                .await
        })
        .await
        .unwrap();

    for (table_name, expected_count) in [
        ("file_action_details", 1_i64),
        ("tool_call_action_details", 1_i64),
        ("command_action_details", 0_i64),
    ] {
        let rows = test_infrastructure
            .infrastructure
            .sql_builder()
            .trusted_raw_expression("COUNT(*) AS row_count")
            .from(table_name)
            .commit()
            .await
            .unwrap();
        assert_eq!(rows[0].try_get::<i64>("row_count").unwrap(), expected_count);
    }
    let file_detail = test_infrastructure
        .infrastructure
        .sql_builder()
        .select([
            "source_path",
            "content_before",
            "content_after",
            "metadata_json",
        ])
        .from_as("file_action_details", "detail")
        .inner_join_as("llm_actions", "action")
        .on("detail.llm_action_id", "action.id")
        .filter(
            "action.message_id = {}",
            [SqlValue::from(message.id.as_uuid())],
        )
        .commit()
        .await;
    assert_eq!(
        file_detail.unwrap()[0]
            .try_get::<String>("source_path")
            .unwrap(),
        "src/main.rs"
    );
    let tool_detail = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["tool_name", "arguments_json"])
        .from_as("tool_call_action_details", "detail")
        .inner_join_as("llm_actions", "action")
        .on("detail.llm_action_id", "action.id")
        .filter(
            "action.message_id = {}",
            [SqlValue::from(message.id.as_uuid())],
        )
        .commit()
        .await
        .unwrap();
    assert_eq!(
        tool_detail[0].try_get::<String>("tool_name").unwrap(),
        "formatter"
    );
    assert_eq!(
        tool_detail[0]
            .try_get::<serde_json::Value>("arguments_json")
            .unwrap(),
        json!({"path": "src/main.rs"})
    );
    let payload_rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["payload_json"])
        .from("action_status_events")
        .inner_join_as("llm_actions", "action")
        .on("action_status_events.llm_action_id", "action.id")
        .filter(
            "action.message_id = {}",
            [SqlValue::from(message.id.as_uuid())],
        )
        .commit()
        .await
        .unwrap();
    assert_eq!(payload_rows.len(), 2);
    assert!(payload_rows.iter().any(|row| {
        row.try_get::<serde_json::Value>("payload_json")
            .map(|payload| payload == json!({"file_result": "updated"}))
            .unwrap_or(false)
    }));
    assert!(payload_rows.iter().any(|row| {
        row.try_get::<serde_json::Value>("payload_json")
            .map(|payload| payload == json!({"failure": "tool unavailable"}))
            .unwrap_or(false)
    }));
}

#[tokio::test]
async fn enforces_the_complete_llm_action_status_transition_matrix() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let transitions = [
        (LlmActionStatus::Pending, LlmActionStatus::Running, true),
        (LlmActionStatus::Pending, LlmActionStatus::Succeeded, true),
        (LlmActionStatus::Pending, LlmActionStatus::Failed, true),
        (LlmActionStatus::Pending, LlmActionStatus::Cancelled, true),
        (LlmActionStatus::Running, LlmActionStatus::Succeeded, true),
        (LlmActionStatus::Running, LlmActionStatus::Failed, true),
        (LlmActionStatus::Running, LlmActionStatus::Cancelled, true),
        (LlmActionStatus::Pending, LlmActionStatus::Pending, false),
        (LlmActionStatus::Running, LlmActionStatus::Pending, false),
        (LlmActionStatus::Running, LlmActionStatus::Running, false),
        (LlmActionStatus::Succeeded, LlmActionStatus::Pending, false),
        (LlmActionStatus::Succeeded, LlmActionStatus::Running, false),
        (
            LlmActionStatus::Succeeded,
            LlmActionStatus::Succeeded,
            false,
        ),
        (LlmActionStatus::Succeeded, LlmActionStatus::Failed, false),
        (
            LlmActionStatus::Succeeded,
            LlmActionStatus::Cancelled,
            false,
        ),
        (LlmActionStatus::Failed, LlmActionStatus::Pending, false),
        (LlmActionStatus::Failed, LlmActionStatus::Running, false),
        (LlmActionStatus::Failed, LlmActionStatus::Succeeded, false),
        (LlmActionStatus::Failed, LlmActionStatus::Failed, false),
        (LlmActionStatus::Failed, LlmActionStatus::Cancelled, false),
        (LlmActionStatus::Cancelled, LlmActionStatus::Pending, false),
        (LlmActionStatus::Cancelled, LlmActionStatus::Running, false),
        (
            LlmActionStatus::Cancelled,
            LlmActionStatus::Succeeded,
            false,
        ),
        (LlmActionStatus::Cancelled, LlmActionStatus::Failed, false),
        (
            LlmActionStatus::Cancelled,
            LlmActionStatus::Cancelled,
            false,
        ),
    ];

    for (initial_status, next_status, should_succeed) in transitions {
        let action = test_infrastructure
            .infrastructure
            .execute_db_actions(async |transaction| {
                let user_message = transaction
                    .add_message_from_user("Transition", Vec::new(), &chat)
                    .await?;
                let llm_message = transaction
                    .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                    .await?;
                transaction
                    .add_llm_action_to_message_by_id(
                        llm_message.id,
                        None,
                        LlmActionDetails::Command(CommandActionDetails {
                            command_text: "echo".to_owned(),
                            working_directory: "/tmp".to_owned(),
                            environment: BTreeMap::new(),
                        }),
                        LlmActionStatusEventInput {
                            status: initial_status,
                            payload: None,
                        },
                    )
                    .await
            })
            .await
            .unwrap();
        let mut transaction = test_infrastructure
            .infrastructure
            .make_transaction()
            .await
            .unwrap();
        let result = transaction
            .append_llm_action_status_event(
                action.id,
                LlmActionStatusEventInput {
                    status: next_status,
                    payload: None,
                },
            )
            .await;
        assert_eq!(result.is_ok(), should_succeed);
        if should_succeed {
            transaction.commit().await.unwrap();
        } else {
            assert_eq!(
                result.unwrap_err().kind(),
                InfrastructureErrorKind::InvalidLlmActionStatusTransition
            );
            transaction.rollback().await.unwrap();
        }
    }
}

#[tokio::test]
async fn completion_rejects_unfinished_actions_and_failure_cancellation_finalize_them() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (in_progress_message, action) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Run", Vec::new(), &chat)
                .await?;
            let in_progress_message = transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                .await?;
            let action = transaction
                .add_llm_action_to_message_by_id(
                    in_progress_message.id,
                    None,
                    LlmActionDetails::Command(CommandActionDetails {
                        command_text: "long-running".to_owned(),
                        working_directory: "/tmp".to_owned(),
                        environment: BTreeMap::new(),
                    }),
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Running,
                        payload: None,
                    },
                )
                .await?;
            Ok((in_progress_message, action))
        })
        .await
        .unwrap();
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let completion_result = transaction
        .complete_llm_message_by_id(in_progress_message.id, "done")
        .await;
    assert_eq!(
        completion_result.unwrap_err().kind(),
        InfrastructureErrorKind::InvalidMessageOperation
    );
    transaction.rollback().await.unwrap();

    let failed_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .mark_llm_message_as_failed_by_id(in_progress_message.id, "failed")
                .await
        })
        .await
        .unwrap()
        .unwrap();
    let infrastructure::MessageRoleMetadata::Llm(failed_metadata) = failed_message.role_metadata
    else {
        panic!("expected LLM metadata");
    };
    assert_eq!(failed_metadata.llm_message_state, LlmMessageState::Failed);
    let failed_events = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["status"])
        .from("action_status_events")
        .filter("llm_action_id = {}", [SqlValue::from(action.id.as_uuid())])
        .commit()
        .await
        .unwrap();
    assert_eq!(failed_events.len(), 2);
    assert_eq!(
        failed_events[1].try_get::<String>("status").unwrap(),
        "cancelled"
    );

    let (cancelled_message, cancelled_action) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Cancel", Vec::new(), &chat)
                .await?;
            let in_progress_message = transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                .await?;
            let action = transaction
                .add_llm_action_to_message_by_id(
                    in_progress_message.id,
                    None,
                    LlmActionDetails::Command(CommandActionDetails {
                        command_text: "cancel-me".to_owned(),
                        working_directory: "/tmp".to_owned(),
                        environment: BTreeMap::new(),
                    }),
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Running,
                        payload: None,
                    },
                )
                .await?;
            Ok((in_progress_message, action))
        })
        .await
        .unwrap();
    let cancelled_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .mark_llm_message_as_cancelled_by_id(cancelled_message.id)
                .await
        })
        .await
        .unwrap()
        .unwrap();
    let infrastructure::MessageRoleMetadata::Llm(cancelled_metadata) =
        cancelled_message.role_metadata
    else {
        panic!("expected LLM metadata");
    };
    assert_eq!(
        cancelled_metadata.llm_message_state,
        LlmMessageState::Cancelled
    );
    let cancelled_event_count = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("COUNT(*) AS event_count")
        .from("action_status_events")
        .filter(
            "llm_action_id = {}",
            [SqlValue::from(cancelled_action.id.as_uuid())],
        )
        .commit()
        .await
        .unwrap()[0]
        .try_get::<i64>("event_count")
        .unwrap();
    assert_eq!(cancelled_event_count, 2);
}

#[tokio::test]
async fn batched_actions_require_nonempty_terminal_status_histories() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let user_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Question", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    let action_details = LlmActionDetails::Command(CommandActionDetails {
        command_text: "echo".to_owned(),
        working_directory: "/tmp".to_owned(),
        environment: BTreeMap::new(),
    });
    let unfinished_batch = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm(
                    "Answer",
                    vec![LlmActionInput {
                        summary: None,
                        details: action_details.clone(),
                        status_events: vec![LlmActionStatusEventInput {
                            status: LlmActionStatus::Running,
                            payload: None,
                        }],
                    }],
                    &user_message,
                    &chat,
                )
                .await
        })
        .await;
    assert_eq!(
        unfinished_batch.unwrap_err().kind(),
        InfrastructureErrorKind::InvalidLlmActionStatusTransition
    );

    let empty_batch = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm(
                    "Answer",
                    vec![LlmActionInput {
                        summary: None,
                        details: action_details,
                        status_events: Vec::new(),
                    }],
                    &user_message,
                    &chat,
                )
                .await
        })
        .await;
    assert_eq!(
        empty_batch.unwrap_err().kind(),
        InfrastructureErrorKind::InvalidLlmActionStatusTransition
    );
}
