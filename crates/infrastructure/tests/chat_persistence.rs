mod support;

use infrastructure::{
    ChatId, ChatInitializationParameters, ChatUpdateOptions, CommandActionDetails,
    LlmActionDetails, LlmActionInput, LlmActionStatus, LlmActionStatusEventInput,
    ProjectInitializationParameters, SqlValue,
};
use std::collections::BTreeMap;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

#[tokio::test]
async fn updates_chat_settings_without_changing_project_settings() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (project, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let updated_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_chat(
                    ChatUpdateOptions::new(chat.id)
                        .with_name("Updated chat")
                        .with_llm_thinking_enabled(true)
                        .with_llm_context_optimization_enabled(true)
                        .with_cpu_usage_percentage(40)
                        .with_gpu_usage_percentage(30),
                )
                .await
        })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated_chat.name, "Updated chat");
    assert!(updated_chat.llm_thinking_enabled);
    assert!(updated_chat.llm_context_optimization_enabled);
    assert_eq!(updated_chat.cpu_usage_percentage, 40);
    assert_eq!(updated_chat.gpu_usage_percentage, 30);
    assert!(
        !test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .unwrap()
            .llm_thinking_enabled
    );
}

#[tokio::test]
async fn deleting_project_cascades_to_chats() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (project, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_project_by_id(project.id).await)
        .await
        .unwrap();

    let remaining_chats = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["id"])
        .from("chats")
        .filter("id = {}", [SqlValue::from(chat.id.as_uuid())])
        .commit()
        .await
        .unwrap();
    assert!(remaining_chats.is_empty());
}

#[tokio::test]
async fn chat_names_are_unique_per_project_and_reusable_across_projects() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_project = transaction
                .create_project(
                    "First chat project",
                    "/projects/first-chat",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            let second_project = transaction
                .create_project(
                    "Second chat project",
                    "/projects/second-chat",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            let first_chat = transaction
                .create_chat_by_project_id(
                    "Shared name",
                    first_project.id,
                    ChatInitializationParameters::default(),
                )
                .await?;
            let second_chat = transaction
                .create_chat(
                    "Shared name",
                    &second_project,
                    ChatInitializationParameters::default(),
                )
                .await?;
            Ok((first_chat, second_chat))
        })
        .await
        .unwrap();
    assert_ne!(result.0.project_id, result.1.project_id);

    let duplicate_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_chat_by_project_id(
                    "Shared name",
                    result.0.project_id,
                    ChatInitializationParameters::default(),
                )
                .await
        })
        .await;
    assert!(duplicate_result.is_err());
}

#[tokio::test]
async fn chat_empty_updates_and_all_setting_overloads_are_covered() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (project, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let unchanged_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_chat(ChatUpdateOptions::new(chat.id))
                .await
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged_chat.updated_at, chat.updated_at);

    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            assert!(transaction.set_llm_thinking_for_chat(true, &chat).await?);
            assert!(
                transaction
                    .set_llm_thinking_for_chat_by_id(false, chat.id)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_context_optimization_for_chat(true, &chat)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_context_optimization_for_chat_by_id(false, chat.id)
                    .await?
            );
            assert!(transaction.set_cpu_usage_for_chat(80, &chat).await?);
            assert!(
                transaction
                    .set_cpu_usage_for_chat_by_id(70, chat.id)
                    .await?
            );
            assert!(transaction.set_gpu_usage_for_chat(60, &chat).await?);
            assert!(
                transaction
                    .set_gpu_usage_for_chat_by_id(50, chat.id)
                    .await?
            );
            assert!(
                !transaction
                    .set_gpu_usage_for_chat_by_id(50, ChatId::new())
                    .await?
            );
            Ok(())
        })
        .await
        .unwrap();

    let invalid_gpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction.set_gpu_usage_for_chat_by_id(101, chat.id).await
        })
        .await;
    assert!(invalid_gpu.is_err());
    let invalid_cpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction.set_cpu_usage_for_chat_by_id(101, chat.id).await
        })
        .await;
    assert!(invalid_cpu.is_err());

    let missing_update = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_chat(ChatUpdateOptions::new(ChatId::new()))
                .await
        })
        .await
        .unwrap();
    assert!(missing_update.is_none());
    assert_eq!(project.name, "Ollatom");
}

#[tokio::test]
async fn deleting_chat_by_entity_and_id_returns_none_for_missing_chat() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let deleted_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_chat(&chat).await)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deleted_chat.id, chat.id);
    let missing_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_chat_by_id(chat.id).await)
        .await
        .unwrap();
    assert!(missing_chat.is_none());
}

#[tokio::test]
async fn deleting_chat_cascades_messages_actions_details_and_events() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm(
                    "Answer",
                    vec![LlmActionInput {
                        summary: None,
                        details: LlmActionDetails::Command(CommandActionDetails {
                            command_text: "echo".to_owned(),
                            working_directory: "/tmp".to_owned(),
                            environment: BTreeMap::new(),
                        }),
                        status_events: vec![LlmActionStatusEventInput {
                            status: LlmActionStatus::Succeeded,
                            payload: None,
                        }],
                    }],
                    &user_message,
                    &chat,
                )
                .await
        })
        .await
        .unwrap();
    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_chat(&chat).await)
        .await
        .unwrap();

    for table_name in [
        "messages",
        "llm_actions",
        "action_status_events",
        "command_action_details",
    ] {
        let rows = test_infrastructure
            .infrastructure
            .sql_builder()
            .trusted_raw_expression("COUNT(*) AS row_count")
            .from(table_name)
            .commit()
            .await
            .unwrap();
        assert_eq!(rows[0].try_get::<i64>("row_count").unwrap(), 0);
    }
}
