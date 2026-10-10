mod support;

use infrastructure::{AttachmentInput, InfrastructureErrorKind};
use serde_json::json;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

fn attachment_input(media_type: Option<String>, content_sha256: Option<String>) -> AttachmentInput {
    AttachmentInput {
        file_name: "image.png".to_owned(),
        media_type,
        byte_length: 4,
        content_sha256,
        storage_reference: "attachments/image.png".to_owned(),
        metadata: json!({}),
    }
}

#[tokio::test]
async fn validates_optional_attachment_media_and_sha256_metadata() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;

    let valid_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user(
                    "Valid attachment",
                    vec![attachment_input(
                        Some("image/png".to_owned()),
                        Some("a".repeat(64)),
                    )],
                    &chat,
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(valid_message.attachments.len(), 1);

    let invalid_attachment_inputs = [
        attachment_input(Some("  ".to_owned()), None),
        attachment_input(Some("image/png".to_owned()), Some("A".repeat(64))),
        attachment_input(Some("image/png".to_owned()), Some("a".repeat(63))),
        attachment_input(
            Some("image/png".to_owned()),
            Some(format!("{}g", "a".repeat(63))),
        ),
    ];

    for invalid_attachment_input in invalid_attachment_inputs {
        let validation_error = test_infrastructure
            .infrastructure
            .execute_db_actions(async |transaction| {
                transaction
                    .add_message_from_user(
                        "Invalid attachment",
                        vec![invalid_attachment_input],
                        &chat,
                    )
                    .await
            })
            .await
            .unwrap_err();
        assert_eq!(
            validation_error.kind(),
            InfrastructureErrorKind::EntityValidationFailed
        );
    }
}
