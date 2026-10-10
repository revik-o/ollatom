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

impl LlmActionDetails {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::FileChange(details) => {
                if details.source_path.trim().is_empty() {
                    return Err("file-change source path must not be blank");
                }

                if details.operation == FileChangeOperation::Rename
                    && details
                        .destination_path
                        .as_deref()
                        .is_none_or(|path| path.trim().is_empty())
                {
                    return Err("rename destination path must not be blank");
                }

                if !details.metadata.is_object() {
                    return Err("file-change metadata must be an object");
                }
            }
            Self::Command(details) => {
                if details.command_text.trim().is_empty() {
                    return Err("command text must not be blank");
                }

                if details.working_directory.trim().is_empty() {
                    return Err("command working directory must not be blank");
                }
            }
            Self::ToolCall(details) => {
                if details.tool_name.trim().is_empty() {
                    return Err("tool name must not be blank");
                }

                if !details.arguments.is_object() {
                    return Err("tool arguments must be an object");
                }
            }
        }
        Ok(())
    }
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
pub struct LlmActionInitialStatus {
    pub status: LlmActionStatus,
    pub payload: Option<Value>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LlmActionInput {
    pub summary: Option<String>,
    pub details: LlmActionDetails,
    pub initial_status: LlmActionStatusEventInput,
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
    pub initial_status: LlmActionInitialStatus,
    pub status_events: Vec<LlmActionStatusEvent>,
    pub created_at: OffsetDateTime,
}
