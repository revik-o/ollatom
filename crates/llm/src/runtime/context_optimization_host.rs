use crate::{
    AskUserRequest, ContextOptimizationOutcome, ContextOptimizationRequest, InvokeSubagentRequest,
    LlmError, ProviderRunHost, QuestionAnswer, RunEvent, StopToken, SubagentOutcome, ToolCall,
    ToolOutput, Usage,
};
use tokio::sync::Mutex;

pub(super) struct ContextOptimizationProviderHost {
    stop_token: StopToken,
    provider_round: Mutex<u16>,
    streamed_text: Mutex<String>,
    latest_usage: Mutex<Usage>,
}

impl ContextOptimizationProviderHost {
    pub(super) fn new(stop_token: StopToken) -> Self {
        Self {
            stop_token,
            provider_round: Mutex::new(0),
            streamed_text: Mutex::new(String::new()),
            latest_usage: Mutex::new(Usage::default()),
        }
    }

    pub(super) async fn complete_response(
        &self,
        context_optimization_response: &mut crate::LlmResponse,
    ) {
        if context_optimization_response.text.is_empty() {
            let streamed_text = self.streamed_text.lock().await;
            context_optimization_response
                .text
                .clone_from(&streamed_text);
        }

        if context_optimization_response.usage == Usage::default() {
            let latest_usage = self.latest_usage.lock().await;
            context_optimization_response
                .usage
                .clone_from(&latest_usage);
        }
    }
}

impl ProviderRunHost for ContextOptimizationProviderHost {
    fn emit(&self, event: RunEvent) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if self.stop_token.is_stopped() {
                return Err(LlmError::Cancelled);
            }

            match event {
                RunEvent::ResponseDelta(response_delta) => {
                    self.streamed_text.lock().await.push_str(&response_delta);
                }
                RunEvent::Usage(usage) => *self.latest_usage.lock().await = usage,
                RunEvent::ReasoningSummaryDelta(_)
                | RunEvent::ModelTraceDelta(_)
                | RunEvent::Warning(_) => {}
                RunEvent::Tool(_)
                | RunEvent::InteractionRequested(_)
                | RunEvent::ProviderRoundStarted { .. }
                | RunEvent::ContextOptimized { .. }
                | RunEvent::Completed
                | RunEvent::Cancelled
                | RunEvent::Failed(_) => {
                    return Err(LlmError::ProviderProtocol(
                        "context optimizer emitted a host-owned event".into(),
                    ));
                }
            }

            Ok(())
        })
    }

    fn begin_round(&self, provider_round: u16) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if self.stop_token.is_stopped() {
                return Err(LlmError::Cancelled);
            }

            let mut current_provider_round = self.provider_round.lock().await;

            if provider_round != 1 || *current_provider_round != 0 {
                return Err(LlmError::LoopLimit(
                    "context optimization permits exactly one provider round".into(),
                ));
            }

            *current_provider_round = provider_round;

            Ok(())
        })
    }

    fn execute_tool_round(
        &self,
        _provider_round: u16,
        _tool_calls: Vec<ToolCall>,
    ) -> crate::BoxFuture<'_, Result<Vec<ToolOutput>, LlmError>> {
        Box::pin(async {
            Err(LlmError::ToolProtocol(
                "context optimization does not expose tools".into(),
            ))
        })
    }

    fn ask_user(
        &self,
        _ask_user_request: AskUserRequest,
    ) -> crate::BoxFuture<'_, Result<Vec<QuestionAnswer>, LlmError>> {
        Box::pin(async {
            Err(LlmError::ToolProtocol(
                "context optimization cannot ask the user".into(),
            ))
        })
    }

    fn invoke_subagent(
        &self,
        _invoke_subagent_request: InvokeSubagentRequest,
    ) -> crate::BoxFuture<'_, Result<SubagentOutcome, LlmError>> {
        Box::pin(async {
            Err(LlmError::ToolProtocol(
                "context optimization cannot invoke a subagent".into(),
            ))
        })
    }

    fn optimize_context(
        &self,
        context_optimization_request: ContextOptimizationRequest,
    ) -> crate::BoxFuture<'_, Result<ContextOptimizationOutcome, LlmError>> {
        Box::pin(async move {
            Ok(ContextOptimizationOutcome {
                model_visible_history: context_optimization_request.history.clone(),
                preserved_history: context_optimization_request.history,
                usage: Usage::default(),
                optimized: false,
            })
        })
    }
}
