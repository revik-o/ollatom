use super::interactions::InteractionHub;
use crate::{
    AskUserRequest, InteractionReply, InteractionRequest, InvokeSubagentRequest, LlmError,
    ProviderRunHost, QuestionAnswer, RunEvent, RunId, RunLimits, RunPolicy, StopToken,
    SubagentOutcome, SubagentProfileRegistry, ToolCall, ToolOutput, ToolRegistry,
    events::EventDispatcher, interaction::InteractionCallback, subagent::SharedSubagentRunner,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::Mutex;

pub(crate) struct RunHost {
    pub run_id: RunId,
    pub events: Arc<EventDispatcher>,
    pub tools: Arc<ToolRegistry>,
    pub selected_tools: BTreeSet<String>,
    pub policy: RunPolicy,
    pub stop: StopToken,
    pub limits: RunLimits,
    pub interactions: Arc<InteractionHub>,
    pub interaction_callback: Option<InteractionCallback>,
    pub profiles: Arc<SubagentProfileRegistry>,
    pub subagent_runner: Option<SharedSubagentRunner>,
    pub authorizer: Option<Arc<dyn crate::ToolAuthorizer>>,
    pub parent_context: Vec<crate::ConversationMessage>,
    pub(super) state: Mutex<HostState>,
    pub(super) execution: Mutex<()>,
}
#[derive(Default)]
pub(super) struct HostState {
    pub(super) round: u16,
    pub(super) total_calls: u16,
    pub(super) children: u8,
    pub(super) completed_calls: BTreeMap<String, (String, serde_json::Value, ToolOutput)>,
    pub(super) child_usage: Vec<crate::Usage>,
    pub(super) tool_records: Vec<crate::ToolExecutionRecord>,
}

impl RunHost {
    pub(super) fn is_tool_selected(&self, tool_name: &str) -> bool {
        self.selected_tools.contains(tool_name)
    }
}

impl ProviderRunHost for RunHost {
    fn emit(&self, event: RunEvent) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if is_host_owned_event(&event) {
                return Err(LlmError::ProviderProtocol(
                    "provider attempted to emit a host-owned run event".into(),
                ));
            }

            self.events.emit(event).await
        })
    }
    fn begin_round(&self, round: u16) -> crate::BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if self.stop.is_stopped() {
                return Err(LlmError::Cancelled);
            }

            let mut state = self.state.lock().await;
            let round_is_out_of_range = round == 0 || round > self.limits.provider_rounds;
            let round_is_not_next = state.round.checked_add(1) != Some(round);

            if round_is_out_of_range || round_is_not_next {
                return Err(LlmError::LoopLimit(format!(
                    "invalid provider round {round}"
                )));
            }

            state.round = round;
            drop(state);

            self.events
                .emit(RunEvent::ProviderRoundStarted { round })
                .await
        })
    }
    fn execute_tool_round(
        &self,
        round: u16,
        tool_calls: Vec<ToolCall>,
    ) -> crate::BoxFuture<'_, Result<Vec<ToolOutput>, LlmError>> {
        Box::pin(async move {
            let _execution_guard = self.execution.lock().await;
            self.validate_tool_round(round, &tool_calls).await?;
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
}

fn is_host_owned_event(event: &RunEvent) -> bool {
    let is_host_operation = matches!(
        event,
        RunEvent::Tool(_)
            | RunEvent::InteractionRequested(_)
            | RunEvent::ProviderRoundStarted { .. }
    );

    is_host_operation || event.is_terminal()
}
