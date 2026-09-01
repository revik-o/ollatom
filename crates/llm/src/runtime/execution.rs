use super::{
    LlmRuntime,
    execution_outcome::{
        HostResults, attach_host_results, cancelled_outcome, empty_cancelled_outcome,
    },
    host::RunHost,
    host_constructor::RunHostDependencies,
    negotiation, request_validation,
};
use crate::{
    LlmError, LlmRun, LlmRunOutcome, ProviderRunOutcome, RunEvent, cancellation::stop_pair,
    events::EventDispatcher,
};
use std::sync::Arc;

pub(crate) fn start_run(mut request_data: crate::request::RequestData) -> LlmRun {
    let (stop_token, stop_handle) = stop_pair();
    let run_id = request_data
        .runtime
        .as_ref()
        .map(LlmRuntime::next_run_id)
        .unwrap_or(crate::RunId(0));
    let event_sink = request_data
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.inner.event_sink.clone());
    let (event_dispatcher, event_stream) =
        EventDispatcher::new(run_id, event_sink, request_data.callbacks.clone());
    let event_dispatcher = Arc::new(event_dispatcher);

    let registration_error = request_data
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.register_run(run_id, stop_handle.clone()).err());
    if let Some(error) = registration_error {
        request_data.validation_error = Some(error);
    }

    let registration_guard = ActiveRunGuard {
        runtime: request_data.runtime.clone(),
        run_id,
    };
    let execution_stop_handle = stop_handle.clone();
    let outcome_future = Box::pin(async move {
        let (outcome_sender, outcome_receiver) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let _registration_guard = registration_guard;
            let run_result = execute_request(
                &mut request_data,
                run_id,
                event_dispatcher.clone(),
                stop_token,
                execution_stop_handle,
            )
            .await;
            let terminal_event = terminal_event_for(&run_result);
            let final_result = match event_dispatcher.emit(terminal_event).await {
                Ok(()) => run_result,
                Err(error) => Err(error),
            };
            let _ignored_receiver = outcome_sender.send(final_result);
        });

        outcome_receiver
            .await
            .map_err(|_| LlmError::Provider("run task terminated without an outcome".into()))?
    });

    LlmRun {
        id: run_id,
        stop_handle,
        outcome_future,
        event_stream: Some(event_stream),
        finished: false,
        detached: false,
    }
}

async fn execute_request(
    request_data: &mut crate::request::RequestData,
    run_id: crate::RunId,
    event_dispatcher: Arc<EventDispatcher>,
    stop_token: crate::StopToken,
    stop_handle: crate::StopHandle,
) -> Result<LlmRunOutcome, LlmError> {
    if let Some(error) = request_data.validation_error.clone() {
        return Err(error);
    }
    let runtime = request_data
        .runtime
        .as_ref()
        .ok_or(LlmError::MissingRuntime)?;
    let provider_id = request_data
        .provider
        .clone()
        .ok_or_else(|| LlmError::InvalidRequest("request does not contain a provider".into()))?;
    let (registration, model_id) =
        runtime.resolve_selected_or_default_model(&provider_id, request_data.model.as_ref())?;
    negotiation::validate_context(&provider_id, &request_data.context)?;
    request_data.limits.validate()?;
    if let Some(user_message) = &request_data.user_message {
        negotiation::validate_user_message(&provider_id, user_message)?;
    }
    if let Some(reasoning_effort) = request_data.explicit_effort {
        request_data.options.reasoning.effort = reasoning_effort;
    }
    let warnings = negotiation::negotiate_options(registration, &mut request_data.options)?;
    for warning in warnings {
        event_dispatcher.emit(RunEvent::Warning(warning)).await?;
    }
    let tool_definitions = request_validation::resolve_selected_tool_definitions(
        runtime,
        &request_data.selected_tools,
    )?;
    request_validation::validate_tool_limits(&request_data.selected_tools, request_data.limits)?;
    request_validation::validate_policy(
        &tool_definitions,
        &request_data.policy,
        request_data.interaction_callback.is_some(),
    )?;
    let run_host = Arc::new(RunHost::new(RunHostDependencies {
        run_id,
        event_dispatcher: event_dispatcher.clone(),
        tool_registry: runtime.inner.tools.clone(),
        selected_tools: request_data.selected_tools.clone(),
        policy: request_data.policy.clone(),
        stop_token: stop_token.clone(),
        limits: request_data.limits,
        interaction_hub: runtime.inner.interactions.clone(),
        interaction_callback: request_data.interaction_callback.clone(),
        subagent_profiles: runtime.inner.profiles.clone(),
        subagent_runner: runtime.inner.subagent_runner.clone(),
        tool_authorizer: runtime.inner.authorizer.clone(),
        parent_context: request_data.context.clone(),
    }));
    let provider_request = crate::ProviderRunRequest {
        run_id,
        provider: provider_id.clone(),
        model: model_id.clone(),
        system_prompt: request_data.system_prompt.clone(),
        context: request_data.context.clone(),
        user_message: request_data
            .user_message
            .clone()
            .ok_or_else(|| LlmError::InvalidRequest("user message is required".into()))?,
        options: request_data.options.clone(),
        tools: tool_definitions,
        limits: request_data.limits,
    };
    if stop_token.is_stopped() {
        return Ok(empty_cancelled_outcome(provider_id, model_id));
    }
    let provider_future =
        registration
            .provider
            .run(provider_request, run_host.clone(), stop_token.clone());
    let provider_result = await_provider(
        provider_future,
        request_data.options.transport.overall_timeout_ms,
        &stop_handle,
    )
    .await;
    if let Err(timeout_error @ LlmError::Timeout(_)) = &provider_result {
        return Err(timeout_error.clone());
    }
    if let Ok(provider_outcome) = &provider_result {
        super::outcome_validation::validate_identity(provider_outcome, &provider_id, &model_id)?;
    }
    if matches!(&provider_result, Err(LlmError::Cancelled)) {
        stop_handle.stop();
    }
    let host_results = HostResults::new(
        run_host.take_child_usage().await,
        run_host.take_tool_records().await,
    );
    if stop_token.is_stopped() {
        return Ok(cancelled_outcome(
            provider_result,
            provider_id,
            model_id,
            &event_dispatcher,
            host_results,
        )
        .await);
    }
    let provider_outcome = attach_host_results(provider_result?, host_results);
    Ok(match provider_outcome {
        ProviderRunOutcome::Completed(response) if stop_token.is_stopped() => {
            LlmRunOutcome::Cancelled(response.into())
        }
        ProviderRunOutcome::Completed(response) => LlmRunOutcome::Completed(response),
        ProviderRunOutcome::Cancelled(partial_response) => {
            LlmRunOutcome::Cancelled(partial_response)
        }
    })
}

async fn await_provider(
    provider_future: crate::BoxFuture<'_, Result<ProviderRunOutcome, LlmError>>,
    overall_timeout_milliseconds: Option<u64>,
    stop_handle: &crate::StopHandle,
) -> Result<ProviderRunOutcome, LlmError> {
    let Some(timeout_milliseconds) = overall_timeout_milliseconds else {
        return provider_future.await;
    };
    match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_milliseconds),
        provider_future,
    )
    .await
    {
        Ok(provider_result) => provider_result,
        Err(_) => {
            stop_handle.stop();
            Err(LlmError::Timeout(format!(
                "LLM run exceeded {timeout_milliseconds}ms"
            )))
        }
    }
}

fn terminal_event_for(result: &Result<LlmRunOutcome, LlmError>) -> RunEvent {
    match result {
        Ok(LlmRunOutcome::Completed(_)) => RunEvent::Completed,
        Ok(LlmRunOutcome::Cancelled(_)) | Err(LlmError::Cancelled) => RunEvent::Cancelled,
        Err(error) => RunEvent::Failed(error.to_string()),
    }
}

struct ActiveRunGuard {
    runtime: Option<LlmRuntime>,
    run_id: crate::RunId,
}
impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        if let Some(runtime) = &self.runtime {
            runtime.unregister_run(self.run_id);
        }
    }
}
