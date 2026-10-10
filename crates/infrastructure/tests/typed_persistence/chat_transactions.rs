use super::*;

#[tokio::test]
async fn typed_chat_repository_updates_and_deletes_chats() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let updated_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_chat(
                    ChatUpdateOptions::new(chat.id)
                        .with_name("Renamed chat")
                        .with_execution_settings(
                            ExecutionSettingsUpdate::default().with_cpu_usage_percentage(75),
                        ),
                )
                .await
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_chat.name, "Renamed chat");
    assert_eq!(updated_chat.execution_settings.cpu_usage_percentage, 75);

    let deleted_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_chat(chat.id).await)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deleted_chat.id, chat.id);
}

#[tokio::test]
async fn typed_message_repository_persists_revisions_and_actions() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let (user_message, llm_message, revised_message) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), &chat)
                .await?;
            let llm_message = transaction
                .add_message_from_llm(
                    "Answer",
                    vec![LlmActionInput {
                        summary: Some("Run command".to_owned()),
                        details: LlmActionDetails::Command(CommandActionDetails {
                            command_text: "echo answer".to_owned(),
                            working_directory: "/tmp".to_owned(),
                            environment: BTreeMap::new(),
                        }),
                        initial_status: LlmActionStatusEventInput {
                            status: LlmActionStatus::Pending,
                            payload: None,
                        },
                        status_events: vec![
                            LlmActionStatusEventInput {
                                status: LlmActionStatus::Running,
                                payload: None,
                            },
                            LlmActionStatusEventInput {
                                status: LlmActionStatus::Succeeded,
                                payload: None,
                            },
                        ],
                    }],
                    &user_message,
                    &chat,
                )
                .await?;
            let revised_message = transaction
                .edit_message_from_user("Revised question", user_message.id)
                .await?
                .unwrap();
            Ok((user_message, llm_message, revised_message))
        })
        .await
        .unwrap();

    let MessageRoleMetadata::User(revision_metadata) = revised_message.role_metadata else {
        panic!("expected user message metadata");
    };
    assert_eq!(revision_metadata.user_revision_number, 2);
    assert_eq!(revised_message.contents, "Revised question");
    assert!(matches!(
        llm_message.role_metadata,
        MessageRoleMetadata::Llm(_)
    ));
    assert_ne!(user_message.id, revised_message.id);
}

#[tokio::test]
async fn failed_typed_transaction_rolls_back_prior_mutations() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Temporary", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_user("", Vec::new(), &chat)
                .await
        })
        .await;
    assert!(result.is_err());

    let replacement = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Replacement", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    assert_eq!(replacement.sequence_number, 1);
}

#[tokio::test]
async fn explicit_transaction_commit_and_rollback_have_distinct_effects() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let mut rolled_back_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    rolled_back_transaction
        .add_message_from_user("Rolled back", Vec::new(), &chat)
        .await
        .unwrap();
    rolled_back_transaction.rollback().await.unwrap();

    let committed_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Committed", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    assert_eq!(committed_message.sequence_number, 1);
}

#[tokio::test]
async fn typed_action_journal_enforces_transitions_before_completion() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let completed_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), &chat)
                .await?;
            let llm_message = transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                .await?;
            let action = transaction
                .add_llm_action_to_message_by_id(
                    llm_message.id,
                    Some("Execute".to_owned()),
                    command_action_details(),
                    status_event(LlmActionStatus::Pending),
                )
                .await?;
            let running_event = transaction
                .append_llm_action_status_event(action.id, status_event(LlmActionStatus::Running))
                .await?;
            assert_eq!(running_event.sequence_number, 2);
            let succeeded_event = transaction
                .append_llm_action_status_event(action.id, status_event(LlmActionStatus::Succeeded))
                .await?;
            assert_eq!(succeeded_event.sequence_number, 3);
            transaction
                .complete_llm_message_by_id(llm_message.id, "Done")
                .await
        })
        .await
        .unwrap()
        .unwrap();

    let MessageRoleMetadata::Llm(metadata) = completed_message.role_metadata else {
        panic!("expected LLM message metadata");
    };
    assert_eq!(metadata.llm_message_state, LlmMessageState::Completed);
}
