mod support;

use infrastructure::{
    AttachmentInput, LlmActionInput, LlmMessageState, MessageRoleMetadata, SqlValue,
};
use serde_json::json;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

#[tokio::test]
async fn creates_user_message_with_attachment_metadata() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user(
                    "Review the attached file",
                    vec![AttachmentInput {
                        file_name: "main.rs".to_owned(),
                        media_type: Some("text/rust".to_owned()),
                        byte_length: 128,
                        content_sha256: Some("hash".to_owned()),
                        storage_reference: "attachments/main.rs".to_owned(),
                        metadata: json!({"language": "rust"}),
                    }],
                    &chat,
                )
                .await
        })
        .await
        .unwrap();

    let MessageRoleMetadata::User(user_metadata) = message.role_metadata else {
        panic!("expected user message metadata");
    };
    assert_eq!(user_metadata.user_revision_number, 1);
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].metadata, json!({"language": "rust"}));
}

#[tokio::test]
async fn creates_multiple_llm_response_rounds_for_same_user_revision() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (first_response, second_response) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Explain this project", Vec::new(), &chat)
                .await?;
            let first_response = transaction
                .add_message_from_llm(
                    "First answer",
                    Vec::<LlmActionInput>::new(),
                    &user_message,
                    &chat,
                )
                .await?;
            let second_response = transaction
                .add_message_from_llm(
                    "Second answer",
                    Vec::<LlmActionInput>::new(),
                    &user_message,
                    &chat,
                )
                .await?;
            Ok((first_response, second_response))
        })
        .await
        .unwrap();

    let MessageRoleMetadata::Llm(first_metadata) = first_response.role_metadata else {
        panic!("expected LLM message metadata");
    };
    let MessageRoleMetadata::Llm(second_metadata) = second_response.role_metadata else {
        panic!("expected LLM message metadata");
    };
    assert_eq!(first_metadata.llm_response_round_number, 1);
    assert_eq!(second_metadata.llm_response_round_number, 2);
    assert_eq!(first_metadata.llm_message_state, LlmMessageState::Completed);
}

#[tokio::test]
async fn rejects_empty_user_message_without_attachments() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let add_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("", Vec::new(), &chat)
                .await
        })
        .await;

    assert!(add_result.is_err());
}

#[tokio::test]
async fn creates_attachment_only_user_message() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user(
                    "",
                    vec![AttachmentInput {
                        file_name: "attachment.txt".to_owned(),
                        media_type: None,
                        byte_length: 1,
                        content_sha256: None,
                        storage_reference: "attachments/attachment.txt".to_owned(),
                        metadata: json!({}),
                    }],
                    &chat,
                )
                .await
        })
        .await
        .unwrap();

    assert!(message.contents.is_empty());
    assert_eq!(message.attachments.len(), 1);
}

#[tokio::test]
async fn sqlite_rejects_user_rows_without_user_revision_metadata() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let timestamp = "2026-08-25T00:00:00Z";
    let insert_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("messages")
        .columns([
            "id",
            "chat_id",
            "sequence_number",
            "role",
            "contents",
            "user_revision_group_id",
            "user_revision_number",
            "llm_reply_to_user_message_id",
            "llm_response_round_number",
            "llm_message_state",
            "validity",
            "created_at",
            "updated_at",
            "deprecated_at",
        ])
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(chat.id.as_uuid()),
            SqlValue::from(1_i64),
            SqlValue::from("user"),
            SqlValue::from("invalid"),
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::from("active"),
            SqlValue::from(timestamp),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .commit()
        .await;

    assert!(insert_result.is_err());
}

#[tokio::test]
async fn stores_null_user_columns_for_llm_messages() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let llm_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm("Answer", Vec::new(), &user_message, &chat)
                .await
        })
        .await
        .unwrap();

    let rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["user_revision_group_id", "user_revision_number"])
        .from("messages")
        .filter("id = {}", [SqlValue::from(llm_message.id.as_uuid())])
        .commit()
        .await
        .unwrap();
    assert_eq!(
        rows[0]
            .try_get::<infrastructure::SqlValue>("user_revision_group_id")
            .unwrap(),
        infrastructure::SqlValue::Null
    );
    assert_eq!(
        rows[0]
            .try_get::<infrastructure::SqlValue>("user_revision_number")
            .unwrap(),
        infrastructure::SqlValue::Null
    );
    let role_specific_columns = test_infrastructure
        .infrastructure
        .sql_builder()
        .select([
            "user_revision_group_id",
            "user_revision_number",
            "llm_reply_to_user_message_id",
            "llm_response_round_number",
            "llm_message_state",
        ])
        .from("messages")
        .filter("id = {}", [SqlValue::from(llm_message.id.as_uuid())])
        .commit()
        .await
        .unwrap();
    assert_eq!(role_specific_columns[0].columns().len(), 5);
}

#[tokio::test]
async fn sqlite_rejects_mixed_llm_metadata_and_invalid_uuid_lengths() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let user_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Target", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    let columns = [
        "id",
        "chat_id",
        "sequence_number",
        "role",
        "contents",
        "user_revision_group_id",
        "user_revision_number",
        "llm_reply_to_user_message_id",
        "llm_response_round_number",
        "llm_message_state",
        "validity",
        "created_at",
        "updated_at",
        "deprecated_at",
    ];
    let mixed_llm_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("messages")
        .columns(columns)
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(chat.id.as_uuid()),
            SqlValue::from(2_i64),
            SqlValue::from("llm"),
            SqlValue::from("invalid"),
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(1_i64),
            SqlValue::from(user_message.id.as_uuid()),
            SqlValue::from(1_i64),
            SqlValue::from("completed"),
            SqlValue::from("active"),
            SqlValue::from("2026-08-25T00:00:00Z"),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .commit()
        .await;
    assert!(mixed_llm_result.is_err());

    let invalid_uuid_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("messages")
        .columns(columns)
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(chat.id.as_uuid()),
            SqlValue::from(2_i64),
            SqlValue::from("user"),
            SqlValue::from("invalid"),
            SqlValue::Blob(vec![1, 2, 3]),
            SqlValue::from(1_i64),
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::from("active"),
            SqlValue::from("2026-08-25T00:00:00Z"),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .commit()
        .await;
    assert!(invalid_uuid_result.is_err());

    let invalid_revision_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("messages")
        .columns(columns)
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(chat.id.as_uuid()),
            SqlValue::from(2_i64),
            SqlValue::from("user"),
            SqlValue::from("invalid"),
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(0_i64),
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::from("active"),
            SqlValue::from("2026-08-25T00:00:00Z"),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .commit()
        .await;
    assert!(invalid_revision_result.is_err());

    let invalid_round_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("messages")
        .columns(columns)
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(chat.id.as_uuid()),
            SqlValue::from(2_i64),
            SqlValue::from("llm"),
            SqlValue::from("invalid"),
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::from(user_message.id.as_uuid()),
            SqlValue::from(0_i64),
            SqlValue::from("completed"),
            SqlValue::from("active"),
            SqlValue::from("2026-08-25T00:00:00Z"),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .commit()
        .await;
    assert!(invalid_round_result.is_err());
}

#[tokio::test]
async fn message_entity_and_id_overloads_are_available() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (user_message, llm_message, revised_message) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user_by_chat_id("Question", Vec::new(), chat.id)
                .await?;
            let llm_message = transaction
                .add_message_from_llm_by_chat_id_and_user_message_id(
                    "Answer",
                    Vec::<LlmActionInput>::new(),
                    chat.id,
                    user_message.id,
                )
                .await?;
            let revised_message = transaction
                .edit_message_from_user_by_id("Edited question", user_message.id)
                .await?
                .unwrap();
            Ok((user_message, llm_message, revised_message))
        })
        .await
        .unwrap();

    assert_eq!(
        user_message.sequence_number,
        revised_message.sequence_number
    );
    assert!(matches!(
        llm_message.role_metadata,
        MessageRoleMetadata::Llm(_)
    ));
}
