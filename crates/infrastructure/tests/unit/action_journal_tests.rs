use crate::{
    Chat, CommandActionDetails, FileChangeDetails, FileChangeOperation, Infrastructure,
    InfrastructureErrorKind, LlmActionDetails, LlmActionId, LlmActionStatus,
    LlmActionStatusEventInput, MessageId,
};
use std::collections::BTreeMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

async fn create_test_chat() -> (TempDir, Infrastructure, Chat) {
    let temporary_directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        temporary_directory.path(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let database_file_path = temporary_directory.path().join("action-journal.sqlite3");
    let project_directory_path = temporary_directory.path().join("journal-project");
    std::fs::create_dir(&project_directory_path).unwrap();
    let infrastructure = Infrastructure::init(database_file_path).await.unwrap();
    let chat = infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Journal project",
                    &project_directory_path,
                    Default::default(),
                )
                .await?;
            transaction
                .create_chat("Journal chat", &project, Default::default())
                .await
        })
        .await
        .unwrap();

    (temporary_directory, infrastructure, chat)
}

async fn create_pending_command_action(
    infrastructure: &Infrastructure,
    chat: &Chat,
) -> (MessageId, MessageId, LlmActionId) {
    infrastructure
        .execute_db_actions(async |transaction| {
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), chat)
                .await?;
            let llm_message = transaction
                .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
                .await?;
            let action = transaction
                .add_llm_action_to_message_by_id(
                    llm_message.id,
                    Some("Run command".to_owned()),
                    LlmActionDetails::Command(CommandActionDetails {
                        command_text: "echo result".to_owned(),
                        working_directory: "/tmp".to_owned(),
                        environment: BTreeMap::new(),
                    }),
                    LlmActionStatusEventInput {
                        status: LlmActionStatus::Pending,
                        payload: None,
                    },
                )
                .await?;
            assert_eq!(action.initial_status.status, LlmActionStatus::Pending);
            assert!(action.status_events.is_empty());
            Ok((user_message.id, llm_message.id, action.id))
        })
        .await
        .unwrap()
}

async fn insert_raw_action(
    infrastructure: &Infrastructure,
    message_id: MessageId,
    sequence_number: u32,
    action_kind: &str,
    action_details_json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO llm_actions (id, message_id, sequence_number, action_kind, action_details_json, summary, initial_status, initial_status_payload_json, initial_status_occurred_at, created_at) VALUES (?, ?, ?, ?, ?, NULL, 'pending', NULL, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
    )
    .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
    .bind(message_id.as_bytes().to_vec())
    .bind(i64::from(sequence_number))
    .bind(action_kind)
    .bind(action_details_json)
    .execute(&infrastructure.connection_pool)
    .await
    .map(|_| ())
}

mod action_validation;
mod aggregate_integrity;
mod sequence_limits;
