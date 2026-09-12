use super::{
    content::{NativeMessage, message_from_content_async},
    streaming::RoundResult,
};
use llm::{ContentBlock, ConversationMessage, ConversationRole, LlmError, ToolFailure, ToolOutput};
use serde_json::json;
use std::collections::HashMap;

pub(super) async fn native_messages_from_history(
    system_prompt: Option<&str>,
    history: &[ConversationMessage],
    model_is_gemma_four: bool,
) -> Result<Vec<NativeMessage>, LlmError> {
    let mut messages = Vec::with_capacity(history.len() + usize::from(system_prompt.is_some()));
    let mut historical_tool_names = HashMap::new();

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
                &mut historical_tool_names,
            )
            .await?,
        );
    }

    Ok(messages)
}

pub(super) fn append_round_history(
    history: &mut Vec<ConversationMessage>,
    round_result: &RoundResult,
    calls: &[llm::ToolCall],
    outputs: &[ToolOutput],
) -> Result<(), LlmError> {
    let mut assistant_content = Vec::new();

    if !round_result.content.is_empty() {
        assistant_content.push(ContentBlock::Text {
            text: round_result.content.clone(),
        });
    }

    if let Some(thinking) = &round_result.thinking {
        assistant_content.push(ContentBlock::ReasoningSummary {
            text: thinking.clone(),
        });
    }

    assistant_content.extend(
        calls
            .iter()
            .cloned()
            .map(|call| ContentBlock::ToolCall { call }),
    );

    if !assistant_content.is_empty() {
        history.push(ConversationMessage {
            role: ConversationRole::Assistant,
            content: assistant_content,
        });
    }

    let outputs_by_call_id = outputs
        .iter()
        .map(|output| (output.call_id.as_str(), output))
        .collect::<HashMap<_, _>>();

    for call in calls {
        let output = outputs_by_call_id.get(call.id.as_str()).ok_or_else(|| {
            LlmError::ToolProtocol(format!("tool output missing for call {}", call.id))
        })?;
        history.push(ConversationMessage {
            role: ConversationRole::Tool,
            content: vec![ContentBlock::ToolResult {
                output: (*output).clone(),
            }],
        });
    }

    Ok(())
}

pub(super) fn tool_output_content(output: &ToolOutput) -> Result<String, LlmError> {
    let value = if let Some(error) = &output.error {
        json!({"error": tool_failure_message(error)})
    } else {
        output.content.clone()
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
