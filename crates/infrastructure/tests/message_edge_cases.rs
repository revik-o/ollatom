mod support;

use infrastructure::{
    AttachmentInput, CommandActionDetails, LlmActionDetails, LlmActionInput, LlmActionStatus,
    LlmActionStatusEventInput, MessageRoleMetadata, SqlValue,
};
use serde_json::json;
use std::collections::BTreeMap;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

#[tokio::test]
async fn llm_replies_require_an_active_user_message_in_the_same_chat() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, first_chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let second_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Second project",
                    "/projects/second",
                    infrastructure::ProjectInitializationParameters::default(),
                )
                .await?;
            transaction
                .create_chat(
                    "Second chat",
                    &project,
                    infrastructure::ChatInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let user_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Question", Vec::new(), &first_chat)
                .await
        })
        .await
        .unwrap();

    let cross_chat_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm("Answer", Vec::new(), &user_message, &second_chat)
                .await
        })
        .await;
    assert!(cross_chat_result.is_err());

    let llm_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm("Answer", Vec::new(), &user_message, &first_chat)
                .await
        })
        .await
        .unwrap();
    let llm_target_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm("Invalid answer", Vec::new(), &llm_message, &first_chat)
                .await
        })
        .await;
    assert!(llm_target_result.is_err());

    let revised_user_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .edit_message_from_user("Revised question", &user_message)
                .await
        })
        .await
        .unwrap()
        .unwrap();
    let deprecated_target_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_llm("Deprecated answer", Vec::new(), &user_message, &first_chat)
                .await
        })
        .await;
    assert!(deprecated_target_result.is_err());
    assert!(matches!(
        revised_user_message.role_metadata,
        MessageRoleMetadata::User(_)
    ));
}

#[tokio::test]
async fn interleaved_llm_replies_keep_rounds_independent_per_user_revision() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (first_round, second_user_round, third_round) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_user = transaction
                .add_message_from_user("First question", Vec::new(), &chat)
                .await?;
            let second_user = transaction
                .add_message_from_user("Second question", Vec::new(), &chat)
                .await?;
            let first_round = transaction
                .add_message_from_llm("First answer", Vec::new(), &first_user, &chat)
                .await?;
            let second_user_round = transaction
                .add_message_from_llm("Second answer", Vec::new(), &second_user, &chat)
                .await?;
            let third_round = transaction
                .add_message_from_llm("Another first answer", Vec::new(), &first_user, &chat)
                .await?;
            Ok((first_round, second_user_round, third_round))
        })
        .await
        .unwrap();

    let MessageRoleMetadata::Llm(first_metadata) = first_round.role_metadata else {
        panic!("expected LLM metadata");
    };
    let MessageRoleMetadata::Llm(second_metadata) = second_user_round.role_metadata else {
        panic!("expected LLM metadata");
    };
    let MessageRoleMetadata::Llm(third_metadata) = third_round.role_metadata else {
        panic!("expected LLM metadata");
    };
    assert_eq!(first_metadata.llm_response_round_number, 1);
    assert_eq!(second_metadata.llm_response_round_number, 1);
    assert_eq!(third_metadata.llm_response_round_number, 2);
}

#[tokio::test]
async fn editing_and_deleting_messages_preserves_attachments_and_cascades_history() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (first_user, revised_second_user) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_user = transaction
                .add_message_from_user("First", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm("First answer", Vec::new(), &first_user, &chat)
                .await?;
            let second_user = transaction
                .add_message_from_user(
                    "Second",
                    vec![AttachmentInput {
                        file_name: "notes.txt".to_owned(),
                        media_type: Some("text/plain".to_owned()),
                        byte_length: 5,
                        content_sha256: Some("sha".to_owned()),
                        storage_reference: "notes.txt".to_owned(),
                        metadata: json!({"kind": "notes"}),
                    }],
                    &chat,
                )
                .await?;
            transaction
                .add_message_from_user("Third", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm(
                    "Third answer",
                    vec![LlmActionInput {
                        summary: Some("command".to_owned()),
                        details: LlmActionDetails::Command(CommandActionDetails {
                            command_text: "echo third".to_owned(),
                            working_directory: "/tmp".to_owned(),
                            environment: BTreeMap::new(),
                        }),
                        status_events: vec![LlmActionStatusEventInput {
                            status: LlmActionStatus::Succeeded,
                            payload: Some(json!({"exit_code": 0})),
                        }],
                    }],
                    &second_user,
                    &chat,
                )
                .await?;
            let revised_second_user = transaction
                .edit_message_from_user("Third revised", &second_user)
                .await?
                .unwrap();
            Ok((first_user, revised_second_user))
        })
        .await
        .unwrap();

    assert_eq!(revised_second_user.attachments.len(), 1);
    assert_eq!(revised_second_user.attachments[0].file_name, "notes.txt");
    assert_eq!(revised_second_user.sequence_number, 3);
    let deleted = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .delete_message_from_user_by_id(revised_second_user.id)
                .await
        })
        .await
        .unwrap();
    assert_eq!(deleted.unwrap().id, revised_second_user.id);

    let remaining_messages = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["id"])
        .from("messages")
        .filter("chat_id = {}", [SqlValue::from(chat.id.as_uuid())])
        .commit()
        .await
        .unwrap();
    assert_eq!(remaining_messages.len(), 2);
    assert!(remaining_messages.iter().any(|row| {
        row.try_get::<Vec<u8>>("id")
            .map(|bytes| bytes == first_user.id.as_uuid().as_bytes())
            .unwrap_or(false)
    }));
    for table_name in [
        "attachments",
        "llm_actions",
        "action_status_events",
        "command_action_details",
        "file_action_details",
        "tool_call_action_details",
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
