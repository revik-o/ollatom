use super::{
    content::{NativeMessage, message_from_content_async},
    content_media::AttachmentBudget,
};
use llm::{ConversationMessage, ConversationRole, LlmError, ToolFailure, ToolOutput};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn native_messages_from_history(
    system_prompt: Option<&str>,
    history: &[ConversationMessage],
    model_is_gemma_four: bool,
) -> Result<Vec<NativeMessage>, LlmError> {
    native_messages_from_history_with_budget(
        system_prompt,
        history,
        model_is_gemma_four,
        &mut AttachmentBudget::default(),
    )
    .await
}

pub(crate) async fn native_messages_from_history_with_budget(
    system_prompt: Option<&str>,
    history: &[ConversationMessage],
    model_is_gemma_four: bool,
    attachment_budget: &mut AttachmentBudget,
) -> Result<Vec<NativeMessage>, LlmError> {
    let mut messages = Vec::with_capacity(history.len());

    if system_prompt.is_some() {
        messages.reserve(1);
    }

    let mut historical_tool_names = BTreeMap::new();
    let mut seen_tool_call_identifiers = BTreeSet::new();

    if let Some(system_prompt) = system_prompt {
        let mut system_message = NativeMessage::new("system");
        system_message.content = system_prompt.to_string();
        messages.push(system_message);
    }

    for conversation_message in history {
        messages.push(
            message_from_content_async(
                conversation_message.role,
                &conversation_message.content,
                model_is_gemma_four && conversation_message.role == ConversationRole::Assistant,
                attachment_budget,
                &mut historical_tool_names,
                &mut seen_tool_call_identifiers,
            )
            .await?,
        );
    }

    if let Some(call_identifier) = historical_tool_names.keys().next() {
        return Err(LlmError::InvalidRequest(format!(
            "tool call {call_identifier} has no matching result"
        )));
    }

    Ok(messages)
}

pub(super) fn tool_output_content(output: &ToolOutput) -> Result<String, LlmError> {
    let value = match output {
        ToolOutput::Success { content, .. } => content.clone(),
        ToolOutput::Failure { error, .. } => {
            json!({"error": tool_failure_message(error)})
        }
    };

    if let Some(string) = value.as_str() {
        Ok(string.into())
    } else {
        serde_json::to_string(&value).map_err(|error| LlmError::ProviderProtocol(error.to_string()))
    }
}

fn tool_failure_message(failure: &ToolFailure) -> String {
    match failure {
        ToolFailure::InvalidArguments(message)
        | ToolFailure::Denied(message)
        | ToolFailure::Execution(message)
        | ToolFailure::Timeout(message) => message.clone(),
    }
}
