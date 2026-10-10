use super::content_media::{AttachmentBudget, add_media, infer_media_type, read_attachment};
use llm::{ContentBlock, ConversationRole, LlmError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeMessage {
    pub(super) role: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) content: String,
    #[serde(rename = "images", skip_serializing_if = "Vec::is_empty")]
    pub(super) media: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) thinking: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) tool_calls: Vec<NativeToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_call_identifier: Option<String>,
}

impl NativeMessage {
    pub(super) fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: String::new(),
            media: Vec::new(),
            thinking: None,
            tool_calls: Vec::new(),
            tool_name: None,
            tool_call_identifier: None,
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
    pub(super) arguments: ToolArgumentAssembly,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum ToolArgumentAssembly {
    StringFragments(String),
    CompleteObject(Map<String, Value>),
    #[serde(skip)]
    #[default]
    Uninitialized,
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

struct ContentConversion<'a> {
    role: ConversationRole,
    omit_thinking: bool,
    attachment_budget: &'a mut AttachmentBudget,
    has_tool_result: bool,
    historical_tool_names: &'a mut BTreeMap<String, String>,
    seen_tool_call_identifiers: &'a mut BTreeSet<String>,
}

fn append_content_block(
    native_message: &mut NativeMessage,
    block: &ContentBlock,
    conversion: &mut ContentConversion<'_>,
) -> Result<(), LlmError> {
    match block {
        ContentBlock::Text { text } if conversion.role != ConversationRole::Tool => {
            native_message.content.push_str(text);
        }
        ContentBlock::Binary {
            media_type, data, ..
        } if conversion.role == ConversationRole::User => {
            add_media(
                native_message,
                conversion.attachment_budget,
                media_type,
                data,
            )?;
        }
        ContentBlock::File { path, .. } => {
            return Err(LlmError::UnsupportedOption(format!(
                "file attachment conversion failed: {path}"
            )));
        }
        ContentBlock::ToolCall { call } if conversion.role == ConversationRole::Assistant => {
            if !call.arguments.is_object() {
                return Err(LlmError::InvalidToolArguments(call.name.clone()));
            }

            if !conversion
                .seen_tool_call_identifiers
                .insert(call.id.clone())
            {
                return Err(LlmError::InvalidRequest(format!(
                    "duplicate historical tool call id {}",
                    call.id
                )));
            }
            conversion
                .historical_tool_names
                .insert(call.id.clone(), call.name.clone());
            native_message.tool_calls.push(NativeToolCall {
                id: Some(call.id.clone()),
                function: NativeFunctionCall {
                    index: None,
                    name: call.name.clone(),
                    arguments: ToolArgumentAssembly::CompleteObject(
                        call.arguments
                            .as_object()
                            .cloned()
                            .ok_or_else(|| LlmError::InvalidToolArguments(call.name.clone()))?,
                    ),
                },
            });
        }
        ContentBlock::ToolResult { output } if conversion.role == ConversationRole::Tool => {
            if conversion.has_tool_result {
                return Err(LlmError::InvalidRequest(
                    "tool messages must contain exactly one tool result".into(),
                ));
            }

            conversion.has_tool_result = true;

            let call_identifier = output.call_id();

            native_message.tool_call_identifier = Some(call_identifier.to_owned());
            native_message.tool_name = Some(
                conversion
                    .historical_tool_names
                    .remove(call_identifier)
                    .ok_or_else(|| {
                        LlmError::InvalidRequest(format!(
                            "tool result {} has no unmatched call",
                            call_identifier
                        ))
                    })?,
            );

            native_message.content = super::content_history::tool_output_content(output)?;
        }
        ContentBlock::ReasoningSummary { text }
            if conversion.role == ConversationRole::Assistant && !conversion.omit_thinking =>
        {
            native_message
                .thinking
                .get_or_insert_with(String::new)
                .push_str(text);
        }
        ContentBlock::ReasoningSummary { .. }
            if conversion.role == ConversationRole::Assistant && conversion.omit_thinking => {}
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

    Ok(())
}

pub(super) async fn message_from_content_async(
    role: ConversationRole,
    blocks: &[ContentBlock],
    omit_thinking: bool,
    attachment_budget: &mut AttachmentBudget,
    historical_tool_names: &mut BTreeMap<String, String>,
    seen_tool_call_identifiers: &mut BTreeSet<String>,
) -> Result<NativeMessage, LlmError> {
    let native_role = match role {
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    };

    let mut native_message = NativeMessage::new(native_role);

    let mut conversion = ContentConversion {
        role,
        omit_thinking,
        attachment_budget,
        has_tool_result: false,
        historical_tool_names,
        seen_tool_call_identifiers,
    };

    for block in blocks {
        if let ContentBlock::File { path, media_type } = block {
            if conversion.role != ConversationRole::User {
                return Err(LlmError::InvalidRequest(
                    "file content is incompatible with its message role".into(),
                ));
            }

            let resolved_media_type = infer_media_type(path, media_type.as_deref())?;
            let attachment_data =
                read_attachment(path, &resolved_media_type, conversion.attachment_budget).await?;

            add_media(
                &mut native_message,
                conversion.attachment_budget,
                &resolved_media_type,
                &attachment_data,
            )?;
        } else {
            append_content_block(&mut native_message, block, &mut conversion)?;
        }
    }

    if conversion.role == ConversationRole::Tool && native_message.tool_call_identifier.is_none() {
        return Err(LlmError::InvalidRequest(
            "tool messages must contain a tool result".into(),
        ));
    }

    Ok(native_message)
}
