use crate::{AttachmentId, ChatId, MessageId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MessageRole {
    User,
    Llm,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MessageValidity {
    Active,
    Deprecated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LlmMessageState {
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserMessageMetadata {
    pub user_revision_group_id: Uuid,
    pub user_revision_number: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmMessageMetadata {
    pub llm_reply_to_user_message_id: MessageId,
    pub llm_response_round_number: u32,
    pub llm_message_state: LlmMessageState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MessageRoleMetadata {
    User(UserMessageMetadata),
    Llm(LlmMessageMetadata),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Attachment {
    pub id: AttachmentId,
    pub message_id: MessageId,
    pub position: u32,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_length: u64,
    pub content_sha256: Option<String>,
    pub storage_reference: String,
    pub metadata: Value,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttachmentInput {
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_length: u64,
    pub content_sha256: Option<String>,
    pub storage_reference: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sequence_number: u64,
    pub contents: String,
    pub attachments: Vec<Attachment>,
    pub role_metadata: MessageRoleMetadata,
    pub validity: MessageValidity,
    pub created_at: OffsetDateTime,
    pub updated_at: Option<OffsetDateTime>,
    pub deprecated_at: Option<OffsetDateTime>,
}

impl Message {
    pub fn role(&self) -> MessageRole {
        match self.role_metadata {
            MessageRoleMetadata::User(_) => MessageRole::User,
            MessageRoleMetadata::Llm(_) => MessageRole::Llm,
        }
    }
}
