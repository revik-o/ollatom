use crate::{ModelId, ProviderId, ToolOutput};
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
    pub fn total_tokens_including_children(&self) -> u64 {
        self.total_tokens.unwrap_or_else(|| {
            self.input_tokens.unwrap_or(0)
                + self.output_tokens.unwrap_or(0)
                + self.reasoning_tokens.unwrap_or(0)
        }) + self
            .child_usage
            .iter()
            .map(Self::total_tokens_including_children)
            .sum::<u64>()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub name: String,
    pub output: ToolOutput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub visible_reasoning: Option<String>,
    pub provider: ProviderId,
    pub model: ModelId,
    pub finish_reason: FinishReason,
    pub tool_executions: Vec<ToolExecutionRecord>,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartialResponse {
    pub text: String,
    pub visible_reasoning: Option<String>,
    pub provider: ProviderId,
    pub model: ModelId,
    pub tool_executions: Vec<ToolExecutionRecord>,
    pub usage: Usage,
}

impl From<LlmResponse> for PartialResponse {
    fn from(value: LlmResponse) -> Self {
        Self {
            text: value.text,
            visible_reasoning: value.visible_reasoning,
            provider: value.provider,
            model: value.model,
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
}
