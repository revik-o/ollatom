use super::{
    content::{NativeMessage, NativeTool},
    provider::OllamaProvider,
    request::context_pressure_reached,
    transport::response_error,
};
use llm::{
    AssistantRound, ContextOptimizationRequest, ConversationMessage, ConversationRole, LlmError, ProviderRunHost,
    ProviderRunRequest, StopToken, ToolCall, ToolOutput, append_tool_round_to_history,
};
use reqwest::{Method, Response};
use serde_json::{Map, Value, json};
use std::sync::Arc;
use std::time::Duration;

pub(super) fn create_native_chat_request_body(
    request: &ProviderRunRequest,
    native_messages: &[NativeMessage],
    native_tools: &[NativeTool],
    native_options: &Map<String, Value>,
    native_reasoning: Option<&Value>,
) -> Result<Value, LlmError> {
    let mut request_body = json!({"model": request.model.as_str(), "messages": native_messages, "stream": true});

    if !native_tools.is_empty() {
        request_body["tools"] =
            serde_json::to_value(native_tools).map_err(|error| LlmError::ProviderProtocol(error.to_string()))?;
    }

    let mut generation_options = native_options.clone();

    for option_name in ["truncate", "shift"] {
        if let Some(option_value) = generation_options.remove(option_name) {
            request_body[option_name] = option_value;
        }
    }

    if !generation_options.is_empty() {
        request_body["options"] = Value::Object(generation_options);
    }

    if let Some(native_reasoning) = native_reasoning {
        request_body["think"] = native_reasoning.clone();
    }

    if let Some(keep_alive_seconds) = request.options.local.keep_alive_seconds {
        request_body["keep_alive"] = json!(keep_alive_seconds);
    }

    Ok(request_body)
}

pub(super) async fn send_native_chat_request(
    provider: &OllamaProvider,
    request: &ProviderRunRequest,
    stop: &StopToken,
    request_body: Value,
) -> Result<Response, LlmError> {
    let request_builder = if let Some(connection_timeout_milliseconds) = request.options.transport.connect_timeout_ms {
        provider.request_with_connection_timeout(
            Method::POST,
            "api/chat",
            Duration::from_millis(connection_timeout_milliseconds),
        )?
    } else {
        provider.request(Method::POST, "api/chat")?
    };

    let send_request = provider.send_request(request_builder.json(&request_body), Some(stop), None);

    let response = if let Some(timeout_milliseconds) = request.options.transport.stream_idle_timeout_ms {
        match stop
            .run_until_cancelled_or_timeout(Duration::from_millis(timeout_milliseconds), send_request)
            .await
        {
            llm::ControlledFutureOutcome::Completed(response) => response?,
            llm::ControlledFutureOutcome::Cancelled => return Err(LlmError::Cancelled),
            llm::ControlledFutureOutcome::TimedOut => {
                return Err(LlmError::Timeout("Ollama stream idle timeout".into()));
            }
        }
    } else {
        send_request.await?
    };

    if !response.status().is_success() {
        return Err(response_error(response.status(), Some(&request.model)));
    }

    Ok(response)
}

pub(super) struct ConversationHistory {
    pub(super) preserved: Vec<ConversationMessage>,
    pub(super) model_visible: Vec<ConversationMessage>,
}

impl ConversationHistory {
    pub(super) fn from_request(request: &ProviderRunRequest) -> Self {
        let mut preserved = request.context.clone();
        preserved.push(ConversationMessage {
            role: ConversationRole::User,
            content: request.user_message.content.clone(),
        });

        Self {
            model_visible: preserved.clone(),
            preserved,
        }
    }

    pub(super) fn append_round(
        &mut self,
        assistant_round: &AssistantRound,
        tool_calls: &[ToolCall],
        tool_outputs: &[ToolOutput],
    ) -> Result<(), LlmError> {
        let preserved_history_length = self.preserved.len();
        append_tool_round_to_history(
            &mut self.preserved,
            assistant_round.text.clone(),
            assistant_round.visible_reasoning.clone(),
            tool_calls.to_vec(),
            tool_outputs.to_vec(),
        )?;
        self.model_visible
            .extend_from_slice(&self.preserved[preserved_history_length..]);
        Ok(())
    }
}

pub(super) async fn optimize_context_if_needed(
    request: &ProviderRunRequest,
    host: &Arc<dyn ProviderRunHost>,
    conversation_history: &mut ConversationHistory,
    input_token_limit: Option<u64>,
    observed_input_tokens: u64,
    estimated_additional_tokens: u64,
) -> Result<bool, LlmError> {
    let Some(input_token_limit) = input_token_limit else {
        return Ok(false);
    };

    if !request.options.context.optimization
        || !context_pressure_reached(observed_input_tokens, estimated_additional_tokens, input_token_limit)
    {
        return Ok(false);
    }

    let optimization_outcome = host
        .optimize_context(ContextOptimizationRequest {
            history: conversation_history.preserved.clone(),
            observed_input_tokens,
            input_token_limit,
        })
        .await?;

    if !optimization_outcome.optimized {
        return Ok(false);
    }

    if !optimization_outcome.preserved_history.is_empty() {
        conversation_history.preserved = optimization_outcome.preserved_history;
    }

    conversation_history.model_visible = optimization_outcome.model_visible_history;

    Ok(true)
}
