use super::content_media::{add_media, infer_media_type, read_attachment};
use llm::{ContentBlock, ConversationRole, LlmError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeMessage {
    pub(super) role: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) images: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) audio: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) thinking: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) tool_calls: Vec<NativeToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_call_id: Option<String>,
}

impl NativeMessage {
    pub(super) fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: String::new(),
            images: Vec::new(),
            audio: Vec::new(),
            thinking: None,
            tool_calls: Vec::new(),
            tool_name: None,
            tool_call_id: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct NativeToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<String>,
    pub(super) function: NativeFunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct NativeFunctionCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) index: Option<usize>,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) arguments: Value,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeTool {
    pub(super) r#type: String,
    pub(super) function: NativeToolFunction,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeToolFunction {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) parameters: Value,
}

fn message_from_content(
    role: ConversationRole,
    blocks: &[ContentBlock],
    omit_thinking: bool,
    historical_tool_names: &mut HashMap<String, String>,
) -> Result<NativeMessage, LlmError> {
    let native_role = match role {
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    };
    let mut native_message = NativeMessage::new(native_role);
    let mut has_tool_result = false;

    for block in blocks {
        match block {
            ContentBlock::Text { text } if role != ConversationRole::Tool => {
                native_message.content.push_str(text);
            }
            ContentBlock::Binary {
                media_type, data, ..
            } if role == ConversationRole::User => {
                add_media(&mut native_message, media_type, data)?;
            }
            ContentBlock::File { path, .. } => {
                return Err(LlmError::UnsupportedOption(format!(
                    "file attachment conversion failed: {path}"
                )));
            }
            ContentBlock::ToolCall { call } if role == ConversationRole::Assistant => {
                if !call.arguments.is_object() {
                    return Err(LlmError::InvalidToolArguments(call.name.clone()));
                }

                historical_tool_names.insert(call.id.clone(), call.name.clone());
                native_message.tool_calls.push(NativeToolCall {
                    id: Some(call.id.clone()),
                    function: NativeFunctionCall {
                        index: None,
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                    },
                });
            }
            ContentBlock::ToolResult { output } if role == ConversationRole::Tool => {
                if has_tool_result {
                    return Err(LlmError::InvalidRequest(
                        "tool messages must contain exactly one tool result".into(),
                    ));
                }

                has_tool_result = true;
                native_message.tool_call_id = Some(output.call_id.clone());
                native_message.tool_name = Some(
                    historical_tool_names
                        .get(&output.call_id)
                        .cloned()
                        .ok_or_else(|| {
                            LlmError::InvalidRequest(format!(
                                "tool result {} has no preceding assistant tool call",
                                output.call_id
                            ))
                        })?,
                );
                native_message.content = super::content_history::tool_output_content(output)?;
            }
            ContentBlock::ReasoningSummary { text }
                if role == ConversationRole::Assistant && !omit_thinking =>
            {
                native_message
                    .thinking
                    .get_or_insert_with(String::new)
                    .push_str(text);
            }
            ContentBlock::ReasoningSummary { .. }
                if role == ConversationRole::Assistant && omit_thinking => {}
            ContentBlock::ProviderOpaque { .. } => {
                return Err(LlmError::UnsupportedOption(
                    "provider_opaque_content".into(),
                ));
            }
            ContentBlock::ToolCall { .. }
            | ContentBlock::Text { .. }
            | ContentBlock::Binary { .. }
            | ContentBlock::ToolResult { .. }
            | ContentBlock::ReasoningSummary { .. } => {
                return Err(LlmError::InvalidRequest(
                    "content block is incompatible with its message role".into(),
                ));
            }
        }
    }

    if role == ConversationRole::Tool && native_message.tool_call_id.is_none() {
        return Err(LlmError::InvalidRequest(
            "tool messages must contain a tool result".into(),
        ));
    }

    Ok(native_message)
}

pub(super) async fn message_from_content_async(
    role: ConversationRole,
    blocks: &[ContentBlock],
    omit_thinking: bool,
    historical_tool_names: &mut HashMap<String, String>,
) -> Result<NativeMessage, LlmError> {
    let mut converted_blocks = Vec::with_capacity(blocks.len());

    for block in blocks {
        if let ContentBlock::File { path, media_type } = block {
            if role != ConversationRole::User {
                return Err(LlmError::InvalidRequest(
                    "file content is incompatible with its message role".into(),
                ));
            }

            let resolved_media_type = infer_media_type(path, media_type.as_deref())?;
            let attachment_data = read_attachment(path, &resolved_media_type).await?;

            converted_blocks.push(ContentBlock::Binary {
                media_type: resolved_media_type,
                filename: Path::new(path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(ToString::to_string),
                data: attachment_data,
            });
        } else {
            converted_blocks.push(block.clone());
        }
    }

    message_from_content(
        role,
        &converted_blocks,
        omit_thinking,
        historical_tool_names,
    )
}
