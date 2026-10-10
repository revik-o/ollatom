use super::{
    provider::OllamaProvider,
    run_context::{continue_after_tool_round, emit_provider_round_finished, prepare_model_run},
    run_request::{create_native_chat_request_body, send_native_chat_request},
    streaming::stream_round,
};
use llm::{
    AssistantRound, FinishReason, LlmError, LlmResponse, PartialResponse, ProviderRunHost,
    ProviderRunOutcome, ProviderRunRequest, StopToken, Usage,
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct RunProgress {
    pub(super) assistant_rounds: Vec<AssistantRound>,
    pub(super) current_round: Option<AssistantRound>,
    pub(super) usage: Usage,
}

impl RunProgress {
    pub(super) fn start_round(&mut self, round_number: u16) {
        self.current_round = Some(AssistantRound {
            round_number,
            ..AssistantRound::default()
        });
    }

    pub(super) fn finish_current_round(&mut self) -> Result<(), LlmError> {
        let Some(current_round) = self.current_round.take() else {
            return Err(LlmError::ProviderProtocol(
                "Ollama completed a provider round that was not started".into(),
            ));
        };

        self.assistant_rounds.push(current_round);
        Ok(())
    }

    fn assistant_round_snapshot(&self) -> Vec<AssistantRound> {
        let mut assistant_rounds = self.assistant_rounds.clone();

        if let Some(current_round) = &self.current_round {
            assistant_rounds.push(current_round.clone());
        }

        assistant_rounds
    }
}

pub(super) async fn model(
    provider: OllamaProvider,
    request: ProviderRunRequest,
    host: Arc<dyn ProviderRunHost>,
    stop: StopToken,
) -> Result<ProviderRunOutcome, LlmError> {
    let mut run_progress = RunProgress::default();

    if stop.is_stopped() {
        return Ok(cancelled_response(&request, run_progress));
    }

    match execute_model_run(&provider, &request, &host, &stop, &mut run_progress).await {
        Err(LlmError::Cancelled) => Ok(cancelled_response(&request, run_progress)),
        result => result,
    }
}

async fn execute_model_run(
    provider: &OllamaProvider,
    request: &ProviderRunRequest,
    host: &Arc<dyn ProviderRunHost>,
    stop: &StopToken,
    run_progress: &mut RunProgress,
) -> Result<ProviderRunOutcome, LlmError> {
    let mut model_run_state = prepare_model_run(provider, request, host, stop).await?;

    for round_number in 1..=request.limits.provider_rounds {
        if stop.is_stopped() {
            return Err(LlmError::Cancelled);
        }

        host.begin_round(round_number).await?;
        run_progress.start_round(round_number);

        let request_body = create_native_chat_request_body(
            request,
            &model_run_state.native_messages,
            &model_run_state.native_tools,
            &model_run_state.native_options,
            model_run_state.native_reasoning.as_ref(),
        )?;
        let response = send_native_chat_request(provider, request, stop, request_body).await?;
        let round_result = {
            let RunProgress {
                current_round,
                usage,
                ..
            } = run_progress;
            let current_round = current_round.as_mut().ok_or_else(|| {
                LlmError::ProviderProtocol("Ollama provider round was not started".into())
            })?;

            stream_round(
                response,
                host,
                stop,
                request.options.transport.stream_idle_timeout_ms,
                current_round,
                usage,
            )
            .await?
        };

        if round_result.tool_calls.is_empty() {
            let current_round = run_progress.current_round.as_ref().ok_or_else(|| {
                LlmError::ProviderProtocol("Ollama provider round was not started".into())
            })?;
            emit_provider_round_finished(host, current_round).await?;
            run_progress.finish_current_round()?;
            return Ok(completed_response(request, run_progress));
        }

        if !matches!(
            round_result.finish_reason,
            FinishReason::Stop | FinishReason::ToolCalls
        ) {
            return Err(LlmError::ProviderProtocol(
                "Ollama returned tool calls with an invalid finish reason".into(),
            ));
        }

        let Some(assistant_round) = run_progress.current_round.as_mut() else {
            return Err(LlmError::ProviderProtocol(
                "Ollama provider round was not started".into(),
            ));
        };

        continue_after_tool_round(
            request,
            host,
            round_number,
            &round_result,
            assistant_round,
            &mut model_run_state,
        )
        .await?;

        run_progress.finish_current_round()?;
    }

    Err(LlmError::LoopLimit(format!(
        "Ollama produced tool calls for all {} provider rounds",
        request.limits.provider_rounds
    )))
}

fn completed_response(
    request: &ProviderRunRequest,
    run_progress: &mut RunProgress,
) -> ProviderRunOutcome {
    ProviderRunOutcome::Completed(LlmResponse {
        provider: request.provider.clone(),
        model: request.model.clone(),
        assistant_rounds: std::mem::take(&mut run_progress.assistant_rounds),
        tool_executions: Vec::new(),
        usage: std::mem::take(&mut run_progress.usage),
    })
}

fn cancelled_response(
    request: &ProviderRunRequest,
    run_progress: RunProgress,
) -> ProviderRunOutcome {
    ProviderRunOutcome::Cancelled(PartialResponse {
        provider: request.provider.clone(),
        model: request.model.clone(),
        assistant_rounds: run_progress.assistant_round_snapshot(),
        tool_executions: Vec::new(),
        usage: run_progress.usage,
    })
}
