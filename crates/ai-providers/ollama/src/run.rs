use super::{
    provider::OllamaProvider,
    run_context::{continue_after_tool_round, prepare_model_run},
    run_request::{create_native_chat_request_body, send_native_chat_request},
    streaming::stream_round,
};
use llm::{
    FinishReason, LlmError, LlmResponse, PartialResponse, ProviderRunHost, ProviderRunOutcome,
    ProviderRunRequest, StopToken, Usage,
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct RunProgress {
    pub(super) response_text: String,
    pub(super) visible_reasoning: String,
    pub(super) usage: Usage,
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

        let request_body = create_native_chat_request_body(
            request,
            &model_run_state.native_messages,
            &model_run_state.native_tools,
            &model_run_state.native_options,
            model_run_state.native_reasoning.as_ref(),
        )?;
        let response = send_native_chat_request(provider, request, stop, request_body).await?;
        let round_result = stream_round(
            response,
            host,
            stop,
            request.options.transport.stream_idle_timeout_ms,
            run_progress,
        )
        .await?;

        if round_result.tool_calls.is_empty() {
            return Ok(completed_response(
                request,
                run_progress,
                round_result.finish_reason,
            ));
        }

        continue_after_tool_round(
            request,
            host,
            round_number,
            round_result,
            &mut model_run_state,
        )
        .await?;
    }

    Err(LlmError::LoopLimit(format!(
        "Ollama produced tool calls for all {} provider rounds",
        request.limits.provider_rounds
    )))
}

fn completed_response(
    request: &ProviderRunRequest,
    run_progress: &mut RunProgress,
    finish_reason: FinishReason,
) -> ProviderRunOutcome {
    ProviderRunOutcome::Completed(LlmResponse {
        text: std::mem::take(&mut run_progress.response_text),
        visible_reasoning: take_nonempty_string(&mut run_progress.visible_reasoning),
        provider: request.provider.clone(),
        model: request.model.clone(),
        finish_reason,
        tool_executions: Vec::new(),
        usage: std::mem::take(&mut run_progress.usage),
    })
}

fn cancelled_response(
    request: &ProviderRunRequest,
    mut run_progress: RunProgress,
) -> ProviderRunOutcome {
    ProviderRunOutcome::Cancelled(PartialResponse {
        text: run_progress.response_text,
        visible_reasoning: take_nonempty_string(&mut run_progress.visible_reasoning),
        provider: request.provider.clone(),
        model: request.model.clone(),
        tool_executions: Vec::new(),
        usage: run_progress.usage,
    })
}

fn take_nonempty_string(value: &mut String) -> Option<String> {
    (!value.is_empty()).then(|| std::mem::take(value))
}
