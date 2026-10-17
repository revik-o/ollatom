use crate::{ContentBlock, LlmError, ProviderId};
use std::collections::BTreeSet;

pub(super) fn validate_context(
    selected_provider: &ProviderId,
    conversation_context: &[crate::ConversationMessage],
) -> Result<(), LlmError> {
    for (message_index, message) in conversation_context.iter().enumerate() {
        if message.content.is_empty() {
            return Err(LlmError::InvalidRequest(format!(
                "conversation message {message_index} has no content"
            )));
        }
        for content_block in &message.content {
            validate_role_content(message.role, content_block, message_index)?;
        }
    }

    validate_tool_history(conversation_context)?;

    validate_provider_opaque_block_ownership(
        selected_provider,
        conversation_context.iter().flat_map(|message| message.content.iter()),
    )
}

fn validate_tool_history(conversation_history: &[crate::ConversationMessage]) -> Result<(), LlmError> {
    let mut seen_call_identifiers = BTreeSet::new();
    let mut unmatched_call_identifiers = BTreeSet::new();

    for message in conversation_history {
        for content_block in &message.content {
            match (message.role, content_block) {
                (crate::ConversationRole::Assistant, ContentBlock::ToolCall { call }) => {
                    if !seen_call_identifiers.insert(call.id.as_str()) {
                        return Err(LlmError::InvalidRequest(format!(
                            "conversation has a duplicate tool call identifier {}",
                            call.id
                        )));
                    }
                    unmatched_call_identifiers.insert(call.id.as_str());
                }
                (crate::ConversationRole::Tool, ContentBlock::ToolResult { output })
                    if !unmatched_call_identifiers.remove(output.call_id()) =>
                {
                    return Err(LlmError::InvalidRequest(format!(
                        "conversation has a tool result without an unmatched call {}",
                        output.call_id()
                    )));
                }
                _ => {}
            }
        }
    }

    if let Some(call_identifier) = unmatched_call_identifiers.first() {
        return Err(LlmError::InvalidRequest(format!(
            "conversation tool call {call_identifier} has no matching result"
        )));
    }

    Ok(())
}

pub(super) fn validate_user_message(
    selected_provider: &ProviderId,
    user_message: &crate::UserMessage,
) -> Result<(), LlmError> {
    if user_message.content.is_empty() {
        return Err(LlmError::InvalidRequest("user message has no content".into()));
    }

    if user_message.content.iter().any(|content_block| {
        matches!(
            content_block,
            ContentBlock::ToolCall { .. } | ContentBlock::ToolResult { .. } | ContentBlock::ReasoningSummary { .. }
        )
    }) {
        return Err(LlmError::InvalidRequest(
            "user message contains content incompatible with the user role".into(),
        ));
    }

    validate_provider_opaque_block_ownership(selected_provider, user_message.content.iter())
}

fn validate_role_content(
    role: crate::ConversationRole,
    content_block: &ContentBlock,
    message_index: usize,
) -> Result<(), LlmError> {
    let valid = match role {
        crate::ConversationRole::User => !matches!(
            content_block,
            ContentBlock::ToolCall { .. } | ContentBlock::ToolResult { .. } | ContentBlock::ReasoningSummary { .. }
        ),
        crate::ConversationRole::Assistant => !matches!(content_block, ContentBlock::ToolResult { .. }),
        crate::ConversationRole::Tool => matches!(content_block, ContentBlock::ToolResult { .. }),
    };

    if valid {
        return Ok(());
    }

    Err(LlmError::InvalidRequest(format!(
        "conversation message {message_index} has content incompatible with its role"
    )))
}

fn validate_provider_opaque_block_ownership<'a>(
    selected_provider: &ProviderId,
    content_blocks: impl IntoIterator<Item = &'a ContentBlock>,
) -> Result<(), LlmError> {
    for content_block in content_blocks {
        let ContentBlock::ProviderOpaque {
            provider: owning_provider,
            ..
        } = content_block
        else {
            continue;
        };

        if owning_provider != selected_provider {
            return Err(LlmError::InvalidRequest(format!(
                "opaque state for {owning_provider} cannot be sent to {selected_provider}"
            )));
        }
    }

    Ok(())
}
