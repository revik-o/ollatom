mod support;

use infrastructure::{LlmActionInput, MessageRoleMetadata, SqlValue};
use support::{create_initialized_test_infrastructure, create_project_with_chat};

#[tokio::test]
async fn editing_user_message_creates_revision_and_deprecates_suffix() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let (original_user_message, revised_user_message) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let original_user_message = transaction
                .add_message_from_user("Original question", Vec::new(), &chat)
                .await?;
            transaction
                .add_message_from_llm(
                    "Original answer",
                    Vec::<LlmActionInput>::new(),
                    &original_user_message,
                    &chat,
                )
                .await?;
            let revised_user_message = transaction
                .edit_message_from_user("Revised question", &original_user_message)
                .await?
                .unwrap();
            Ok((original_user_message, revised_user_message))
        })
        .await
        .unwrap();

    let MessageRoleMetadata::User(original_metadata) = original_user_message.role_metadata else {
        panic!("expected original user metadata");
    };
    let MessageRoleMetadata::User(revised_metadata) = revised_user_message.role_metadata else {
        panic!("expected revised user metadata");
    };
    assert_eq!(
        revised_metadata.user_revision_group_id,
        original_metadata.user_revision_group_id
    );
    assert_eq!(revised_metadata.user_revision_number, 2);
    assert_eq!(
        revised_user_message.sequence_number,
        original_user_message.sequence_number
    );
    assert_eq!(
        revised_user_message.created_at,
        original_user_message.created_at
    );
    assert!(revised_user_message.updated_at.is_some());

    let deprecated_message_count = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("COUNT(*) AS deprecated_count")
        .from("messages")
        .filter(
            "chat_id = {} AND validity = {}",
            [
                SqlValue::from(chat.id.as_uuid()),
                SqlValue::from("deprecated"),
            ],
        )
        .commit()
        .await
        .unwrap()[0]
        .try_get::<i64>("deprecated_count")
        .unwrap();
    assert_eq!(deprecated_message_count, 2);
}

#[tokio::test]
async fn deleting_user_message_removes_active_and_deprecated_suffix() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_user_message = transaction
                .add_message_from_user("First", Vec::new(), &chat)
                .await?;
            let second_user_message = transaction
                .add_message_from_user("Second", Vec::new(), &chat)
                .await?;
            transaction
                .edit_message_from_user("Second revised", &second_user_message)
                .await?;
            transaction
                .delete_message_from_user(&first_user_message)
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    let remaining_message_count = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("COUNT(*) AS message_count")
        .from("messages")
        .filter("chat_id = {}", [SqlValue::from(chat.id.as_uuid())])
        .commit()
        .await
        .unwrap()[0]
        .try_get::<i64>("message_count")
        .unwrap();
    assert_eq!(remaining_message_count, 0);
}
