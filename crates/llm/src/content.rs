use crate::{ProviderId, ToolCall, ToolOutput};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Binary {
        media_type: String,
        filename: Option<String>,
        data: Vec<u8>,
    },
    File {
        path: String,
        media_type: Option<String>,
    },
    ToolCall {
        call: ToolCall,
    },
    ToolResult {
        output: ToolOutput,
    },
    ReasoningSummary {
        text: String,
    },
    ProviderOpaque {
        provider: ProviderId,
        kind: String,
        data: Vec<u8>,
    },
}

impl ContentBlock {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text { text: value.into() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: ConversationRole,
    pub content: Vec<ContentBlock>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: Vec<ContentBlock>,
}

impl UserMessage {
    pub fn new(content: impl IntoIterator<Item = ContentBlock>) -> Self {
        Self {
            content: content.into_iter().collect(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        Self::new([ContentBlock::text(value)])
    }
}

impl From<String> for UserMessage {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}
