use super::RunHost;
use crate::{
    AskUserRequest, ContextOptimizationOutcome, ContextOptimizationRequest, InteractionReply,
    InteractionRequest, InvokeSubagentRequest, LlmError, ProviderRunHost, QuestionAnswer, RunEvent,
    SubagentOutcome, ToolCall, ToolOutput, Usage,
};

impl ProviderRunHost for RunHost {
    fn emit(&self, event: RunEvent) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            self.ensure_active()?;

            if is_host_owned_event(&event) {
                return Err(LlmError::ProviderProtocol(
                    "provider attempted to emit a host-owned run event".into(),
                ));
            }

            if let RunEvent::ProviderRoundFinished { round, .. } = &event {
                let state = self.state.lock().await;

                if state.round != *round {
                    return Err(LlmError::ProviderProtocol(
                        "provider finished a round that was not started".into(),
                    ));
                }
            }

            self.events.emit(event).await
        })
    }

    fn begin_round(&self, round: u16) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            let _round_transition_guard = self.execution.lock().await;

            self.ensure_active()?;
            self.flush_pending_finished_events().await?;

            let state = self.state.lock().await;
            let round_is_out_of_range = round == 0 || round > self.limits.provider_rounds;
            let round_is_not_next = state.round.checked_add(1) != Some(round);

            if round_is_out_of_range || round_is_not_next {
                return Err(LlmError::LoopLimit(format!(
                    "invalid provider round {round}"
                )));
            }

            drop(state);

            let (durable_acknowledged, delivery_result) = self
                .events
                .emit_with_durable_ack(RunEvent::ProviderRoundStarted { round })
                .await;

            if durable_acknowledged {
                let mut state = self.state.lock().await;
                state.round = round;
                state.tool_round_executed = false;
            }

            delivery_result
        })
    }

    fn execute_tool_round(
        &self,
        round: u16,
        tool_calls: Vec<ToolCall>,
    ) -> crate::BoxFuture<'_, Result<Vec<ToolOutput>, LlmError>> {
        Box::pin(async move {
            self.ensure_active()?;
            let _tool_execution_guard = self.execution.lock().await;
            let mut state = self.state.lock().await;

            if state.round == round && state.tool_round_executed {
                return Err(LlmError::ProviderProtocol(format!(
                    "tool round {round} was already executed"
                )));
            }

            state.tool_round_executed = true;

            drop(state);

            self.validate_tool_round(round, &tool_calls).await?;
            self.preflight_tool_round(&tool_calls).await?;

            let mut tool_outputs = Vec::with_capacity(tool_calls.len());

            for tool_call in tool_calls {
                tool_outputs.push(self.execute_tool_call(tool_call).await?);
            }

            Ok(tool_outputs)
        })
    }

    fn ask_user(
        &self,
        request: AskUserRequest,
    ) -> crate::BoxFuture<'_, Result<Vec<QuestionAnswer>, LlmError>> {
        Box::pin(async move {
            if !self.is_tool_selected(crate::builtins::ASK_USER_TOOL_NAME) {
                return Err(LlmError::ProviderProtocol(
                    "ask_user was not advertised for this run".into(),
                ));
            }

            request.validate()?;

            let expected_request = request.clone();
            let create_interaction_request = |interaction_id| InteractionRequest::AskUser {
                id: interaction_id,
                request,
            };

            let interaction_reply = if let Some(callback) = &self.interaction_callback {
                tokio::select! {
                    _ = self.stop.cancelled() => Err(LlmError::Cancelled),
                    reply = callback(create_interaction_request(crate::InteractionId(0))) => Ok(reply),
                }?
            } else {
                self.interactions
                    .request_interaction(create_interaction_request, &self.events, &self.stop)
                    .await?
            };

            let InteractionReply::UserAnswers(question_answers) = interaction_reply else {
                return Err(LlmError::ProviderProtocol(
                    "ask_user received an approval reply".into(),
                ));
            };

            expected_request.validate_answers(&question_answers)?;

            Ok(question_answers)
        })
    }
    fn invoke_subagent(
        &self,
        request: InvokeSubagentRequest,
    ) -> crate::BoxFuture<'_, Result<SubagentOutcome, LlmError>> {
        Box::pin(async move {
            if !self.is_tool_selected(crate::builtins::INVOKE_SUBAGENT_TOOL_NAME) {
                return Err(LlmError::ProviderProtocol(
                    "invoke_subagent was not advertised for this run".into(),
                ));
            }

            request.validate()?;

            self.invoke_configured_subagent(request).await
        })
    }

    fn optimize_context(
        &self,
        context_optimization_request: ContextOptimizationRequest,
    ) -> crate::BoxFuture<'_, Result<ContextOptimizationOutcome, LlmError>> {
        Box::pin(async move {
            let _context_optimization_execution_guard = self.execution.lock().await;
            let original_message_count = context_optimization_request.history.len();
            let observed_input_tokens = context_optimization_request.observed_input_tokens;
            let input_token_limit = context_optimization_request.input_token_limit;
            let context_optimization_outcome = self
                .context_optimizer
                .optimize(context_optimization_request)
                .await?;

            if context_optimization_outcome.usage != Usage::default() {
                self.state
                    .lock()
                    .await
                    .child_usage
                    .push(context_optimization_outcome.usage.clone());
            }

            if context_optimization_outcome.optimized {
                self.events
                    .emit(RunEvent::ContextOptimized {
                        original_message_count,
                        optimized_message_count: context_optimization_outcome
                            .model_visible_history
                            .len(),
                        observed_input_tokens,
                        input_token_limit,
                    })
                    .await?;
            }

            Ok(context_optimization_outcome)
        })
    }
}

fn is_host_owned_event(event: &RunEvent) -> bool {
    let is_host_operation = matches!(
        event,
        RunEvent::Tool(_)
            | RunEvent::InteractionRequested(_)
            | RunEvent::ProviderRoundStarted { .. }
            | RunEvent::ContextOptimized { .. }
    );

    is_host_operation || event.is_terminal()
}
