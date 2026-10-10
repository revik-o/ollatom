use crate::{ContentBlock, ConversationMessage, ConversationRole, LlmError, ToolCall, ToolOutput};
use std::collections::{BTreeMap, BTreeSet};

pub fn append_tool_round_to_history(
    history: &mut Vec<ConversationMessage>,
    response_text: String,
    reasoning_summary: Option<String>,
    tool_calls: Vec<ToolCall>,
    tool_outputs: Vec<ToolOutput>,
) -> Result<(), LlmError> {
    let expected_call_identifiers = tool_calls
        .iter()
        .map(|tool_call| tool_call.id.as_str())
        .collect::<BTreeSet<_>>();

    if expected_call_identifiers.len() != tool_calls.len() {
        return Err(LlmError::ToolProtocol(
            "assistant tool call identifiers must be unique".into(),
        ));
    }

    let mut outputs_by_call_identifier = BTreeMap::new();

    for tool_output in tool_outputs {
        let call_identifier = tool_output.call_id();

        if !expected_call_identifiers.contains(call_identifier) {
            return Err(LlmError::ToolProtocol(format!(
                "unexpected tool output for call {call_identifier}"
            )));
        }

        if outputs_by_call_identifier.contains_key(call_identifier) {
            return Err(LlmError::ToolProtocol(format!(
                "duplicate tool output for call {call_identifier}"
            )));
        }

        outputs_by_call_identifier.insert(call_identifier.to_owned(), tool_output);
    }

    for tool_call in &tool_calls {
        if !outputs_by_call_identifier.contains_key(&tool_call.id) {
            return Err(LlmError::ToolProtocol(format!(
                "tool output missing for call {}",
                tool_call.id
            )));
        }
    }

    let mut round_messages = Vec::new();
    let mut assistant_content = Vec::new();

    if !response_text.is_empty() {
        assistant_content.push(ContentBlock::text(response_text));
    }

    if let Some(reasoning_summary) = reasoning_summary.filter(|value| !value.is_empty()) {
        assistant_content.push(ContentBlock::ReasoningSummary {
            text: reasoning_summary,
        });
    }

    for tool_call in tool_calls {
        assistant_content.push(ContentBlock::ToolCall {
            call: tool_call.clone(),
        });

        let tool_output = outputs_by_call_identifier
            .remove(&tool_call.id)
            .ok_or_else(|| {
                LlmError::ToolProtocol(format!("tool output missing for call {}", tool_call.id))
            })?;

        round_messages.push(ConversationMessage {
            role: ConversationRole::Tool,
            content: vec![ContentBlock::ToolResult {
                output: tool_output,
            }],
        });
    }

    if !assistant_content.is_empty() {
        round_messages.insert(
            0,
            ConversationMessage {
                role: ConversationRole::Assistant,
                content: assistant_content,
            },
        );
    }

    history.extend(round_messages);

    Ok(())
}
