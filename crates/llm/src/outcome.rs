use crate::{LlmError, ModelId, ProviderId, ToolCall, ToolOutput};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    ApiReported,
    Estimated,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    #[serde(default)]
    pub source: UsageSource,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub child_usage: Vec<Usage>,
}

impl Usage {
    pub fn total_tokens_including_children(&self) -> Result<u64, UsageOverflow> {
        let own_token_count = match self.total_tokens {
            Some(total_token_count) => total_token_count,
            None => self
                .input_tokens
                .unwrap_or(0)
                .checked_add(self.output_tokens.unwrap_or(0))
                .and_then(|token_count| token_count.checked_add(self.reasoning_tokens.unwrap_or(0)))
                .ok_or(UsageOverflow)?,
        };

        self.child_usage
            .iter()
            .try_fold(own_token_count, |accumulated_token_count, child_usage| {
                accumulated_token_count
                    .checked_add(child_usage.total_tokens_including_children()?)
                    .ok_or(UsageOverflow)
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsageOverflow;

impl std::fmt::Display for UsageOverflow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("token usage exceeds the supported range")
    }
}

impl std::error::Error for UsageOverflow {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Cancelled,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssistantRound {
    pub round_number: u16,
    pub text: String,
    pub visible_reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub name: String,
    pub output: ToolOutput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub provider: ProviderId,
    pub model: ModelId,
    pub assistant_rounds: Vec<AssistantRound>,
    pub tool_executions: Vec<ToolExecutionRecord>,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartialResponse {
    pub provider: ProviderId,
    pub model: ModelId,
    pub assistant_rounds: Vec<AssistantRound>,
    pub tool_executions: Vec<ToolExecutionRecord>,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FailedRun {
    pub error: LlmError,
    pub partial_response: PartialResponse,
    pub terminal_event_delivered: bool,
}

impl From<LlmResponse> for PartialResponse {
    fn from(value: LlmResponse) -> Self {
        Self {
            provider: value.provider,
            model: value.model,
            assistant_rounds: value.assistant_rounds,
            tool_executions: value.tool_executions,
            usage: value.usage,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "response", rename_all = "snake_case")]
pub enum LlmRunOutcome {
    Completed(LlmResponse),
    Cancelled(PartialResponse),
    Failed(FailedRun),
}
