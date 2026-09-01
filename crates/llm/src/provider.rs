use crate::{
    AskUserRequest, AvailabilityReport, BoxFuture, ConversationMessage, InvokeSubagentRequest,
    LlmError, LlmOptions, LlmResponse, ModelId, ModelInfo, ModelScope, PartialResponse,
    ProviderCapabilities, ProviderId, QuestionAnswer, RunEvent, RunId, StopToken, SubagentOutcome,
    ToolCall, ToolDefinition, ToolOutput, UserMessage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLimits {
    pub provider_rounds: u16,
    pub total_tool_calls: u16,
    pub tool_calls_per_round: u16,
    pub child_subagents: u8,
    pub subagent_depth: u8,
    pub tool_timeout_ms: u64,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            provider_rounds: 16,
            total_tool_calls: 64,
            tool_calls_per_round: 16,
            child_subagents: 4,
            subagent_depth: 1,
            tool_timeout_ms: 60_000,
        }
    }
}

impl RunLimits {
    pub fn validate(self) -> Result<(), LlmError> {
        if self.provider_rounds == 0 || self.tool_timeout_ms == 0 {
            return Err(LlmError::InvalidRequest(
                "provider rounds and tool timeout must be greater than zero".into(),
            ));
        }

        if self.tool_calls_per_round > self.total_tool_calls {
            return Err(LlmError::InvalidRequest(
                "per-round tool calls cannot exceed the total tool-call limit".into(),
            ));
        }

        if (self.child_subagents == 0) != (self.subagent_depth == 0) {
            return Err(LlmError::InvalidRequest(
                "subagent count and depth must either both be zero or both be non-zero".into(),
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderRunRequest {
    pub run_id: RunId,
    pub provider: ProviderId,
    pub model: ModelId,
    pub system_prompt: Option<String>,
    pub context: Vec<ConversationMessage>,
    pub user_message: UserMessage,
    pub options: LlmOptions,
    pub tools: Vec<ToolDefinition>,
    pub limits: RunLimits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", content = "response", rename_all = "snake_case")]
pub enum ProviderRunOutcome {
    Completed(LlmResponse),
    Cancelled(PartialResponse),
}

pub trait ProviderRunHost: Send + Sync {
    fn emit(&self, event: RunEvent) -> BoxFuture<'_, Result<(), LlmError>>;

    fn begin_round(&self, round: u16) -> BoxFuture<'_, Result<(), LlmError>>;

    fn execute_tool_round(
        &self,
        round: u16,
        calls: Vec<ToolCall>,
    ) -> BoxFuture<'_, Result<Vec<ToolOutput>, LlmError>>;

    fn ask_user(
        &self,
        request: AskUserRequest,
    ) -> BoxFuture<'_, Result<Vec<QuestionAnswer>, LlmError>>;

    fn invoke_subagent(
        &self,
        request: InvokeSubagentRequest,
    ) -> BoxFuture<'_, Result<SubagentOutcome, LlmError>>;
}

pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &ProviderId;

    fn capabilities(&self) -> ProviderCapabilities;

    fn availability(
        &self,
        model: Option<&ModelId>,
    ) -> BoxFuture<'_, Result<AvailabilityReport, LlmError>>;

    fn list_models(&self, scope: ModelScope) -> BoxFuture<'_, Result<Vec<ModelInfo>, LlmError>>;

    fn model_info(&self, model: &ModelId) -> BoxFuture<'_, Result<ModelInfo, LlmError>>;

    fn run(
        &self,
        request: ProviderRunRequest,
        host: Arc<dyn ProviderRunHost>,
        stop: StopToken,
    ) -> BoxFuture<'_, Result<ProviderRunOutcome, LlmError>>;
}
