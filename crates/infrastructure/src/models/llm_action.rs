use crate::{LlmActionId, LlmActionStatusEventId, MessageId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FileChangeOperation {
    Create,
    Modify,
    Delete,
    Rename,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileChangeDetails {
    pub operation: FileChangeOperation,
    pub source_path: String,
    pub destination_path: Option<String>,
    pub content_before: Option<String>,
    pub content_after: Option<String>,
    pub unified_diff: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandActionDetails {
    pub command_text: String,
    pub working_directory: String,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolCallActionDetails {
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LlmActionDetails {
    FileChange(FileChangeDetails),
    Command(CommandActionDetails),
    ToolCall(ToolCallActionDetails),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LlmActionStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl LlmActionStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmActionStatusEventInput {
    pub status: LlmActionStatus,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmActionInput {
    pub summary: Option<String>,
    pub details: LlmActionDetails,
    pub status_events: Vec<LlmActionStatusEventInput>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmActionStatusEvent {
    pub id: LlmActionStatusEventId,
    pub llm_action_id: LlmActionId,
    pub sequence_number: u32,
    pub status: LlmActionStatus,
    pub payload: Option<Value>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmAction {
    pub id: LlmActionId,
    pub message_id: MessageId,
    pub sequence_number: u32,
    pub summary: Option<String>,
    pub details: LlmActionDetails,
    pub status_events: Vec<LlmActionStatusEvent>,
    pub created_at: OffsetDateTime,
}
