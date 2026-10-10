use crate::{
    AskUserRequest, AssistantRound, ContextOptimizationOutcome, ContextOptimizationRequest,
    FinishReason, InvokeSubagentRequest, LlmError, ProviderRunHost, QuestionAnswer, RunEvent,
    StopToken, SubagentOutcome, ToolCall, ToolOutput, Usage,
};
use tokio::sync::Mutex;

pub(super) struct ContextOptimizationProviderHost {
    stop_token: StopToken,
    streamed_round: Mutex<AssistantRound>,
    latest_usage: Mutex<Usage>,
}

impl ContextOptimizationProviderHost {
    pub(super) fn new(stop_token: StopToken) -> Self {
        Self {
            stop_token,
            streamed_round: Mutex::new(AssistantRound::default()),
            latest_usage: Mutex::new(Usage::default()),
        }
    }

    pub(super) async fn complete_response(
        &self,
        context_optimization_response: &mut crate::LlmResponse,
    ) {
        let streamed_round = self.streamed_round.lock().await.clone();
        let latest_usage = self.latest_usage.lock().await.clone();

        if context_optimization_response.assistant_rounds.is_empty()
            && (streamed_round.round_number != 0
                || !streamed_round.text.is_empty()
                || streamed_round.visible_reasoning.is_some())
        {
            let mut streamed_round = streamed_round;

            if streamed_round.usage == Usage::default() {
                streamed_round.usage = latest_usage.clone();
            }

            context_optimization_response
                .assistant_rounds
                .push(streamed_round);
        } else if let Some(assistant_round) =
            context_optimization_response.assistant_rounds.last_mut()
        {
            if assistant_round.text.is_empty() {
                assistant_round.text = streamed_round.text;
            }

            if assistant_round.visible_reasoning.is_none() {
                assistant_round.visible_reasoning = streamed_round.visible_reasoning;
            }

            if assistant_round.finish_reason == FinishReason::Unknown {
                assistant_round.finish_reason = streamed_round.finish_reason;
            }

            if assistant_round.usage == Usage::default() {
                assistant_round.usage = if streamed_round.usage == Usage::default() {
                    latest_usage.clone()
                } else {
                    streamed_round.usage
                };
            }
        }

        if context_optimization_response.usage == Usage::default() {
            context_optimization_response.usage = latest_usage;
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
                    self.streamed_round
                        .lock()
                        .await
                        .text
                        .push_str(&response_delta);
                }
                RunEvent::ReasoningSummaryDelta(reasoning_delta) => {
                    self.streamed_round
                        .lock()
                        .await
                        .visible_reasoning
                        .get_or_insert_with(String::new)
                        .push_str(&reasoning_delta);
                }
                RunEvent::Usage(usage) => *self.latest_usage.lock().await = usage,
                RunEvent::ProviderRoundFinished {
                    round,
                    finish_reason,
                    tool_calls,
                    usage,
                } => {
                    let mut streamed_round = self.streamed_round.lock().await;
                    if streamed_round.round_number != round {
                        return Err(LlmError::ProviderProtocol(
                            "context optimizer finished a round that was not started".into(),
                        ));
                    }
                    streamed_round.finish_reason = finish_reason;
                    streamed_round.tool_calls = tool_calls;
                    streamed_round.usage = usage;
                }
                RunEvent::ModelTraceDelta(_) | RunEvent::Warning(_) => {}
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

            let mut streamed_round = self.streamed_round.lock().await;

            if provider_round != 1 || streamed_round.round_number != 0 {
                return Err(LlmError::LoopLimit(
                    "context optimization permits exactly one provider round".into(),
                ));
            }

            *streamed_round = AssistantRound {
                round_number: provider_round,
                ..AssistantRound::default()
            };

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
