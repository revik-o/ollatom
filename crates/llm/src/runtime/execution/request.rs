use super::super::{
    context_optimization::ContextOptimizer,
    execution_outcome::{HostResults, attach_host_results, cancelled_outcome, empty_cancelled_outcome, failed_outcome},
    host::RunHost,
    host_constructor::RunHostDependencies,
    negotiation, request_validation,
};
use crate::{LlmError, LlmRunOutcome, ProviderRunOutcome, RunEvent, events::EventDispatcher};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) async fn execute_request(
    request_data: &mut crate::request::RequestData,
    run_id: crate::RunId,
    event_dispatcher: Arc<EventDispatcher>,
    stop_token: crate::StopToken,
    stop_handle: crate::StopHandle,
    overall_deadline: Option<Instant>,
) -> Result<LlmRunOutcome, LlmError> {
    if let Some(error) = request_data.validation_error.clone() {
        return Err(error);
    }

    let runtime = request_data.runtime.as_ref().ok_or(LlmError::MissingRuntime)?;

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

    let tool_definitions = request_validation::resolve_selected_tool_definitions(runtime, &request_data.selected_tools)?;

    request_validation::validate_tool_limits(&request_data.selected_tools, request_data.limits)?;

    let has_approval_path = runtime.inner.authorizer.is_some()
        || request_data.interaction_callback.is_some()
        || request_data.policy.approval().is_some()
        || request_data.policy.streams_approvals();

    request_validation::validate_policy(&tool_definitions, has_approval_path)?;

    let run_host = Arc::new(RunHost::new(RunHostDependencies {
        run_id,
        event_dispatcher: event_dispatcher.clone(),
        tool_registry: runtime.inner.tools.clone(),
        selected_tools: request_data.selected_tools.clone(),
        policy: request_data.policy.clone(),
        stop_token: stop_token.clone(),
        limits: request_data.limits,
        overall_deadline,
        interaction_hub: runtime.inner.interactions.clone(),
        interaction_callback: request_data.interaction_callback.clone(),
        subagent_profiles: runtime.inner.profiles.clone(),
        subagent_runner: runtime.inner.subagent_runner.clone(),
        tool_authorizer: runtime.inner.authorizer.clone(),
        parent_context: request_data.context.clone(),
        context_optimizer: ContextOptimizer::new(
            registration.provider.clone(),
            provider_id.clone(),
            model_id.clone(),
            run_id,
            request_data.system_prompt.clone(),
            request_data.options.clone(),
            stop_token.clone(),
        ),
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

    let provider_timeout = overall_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));

    let provider_result = if provider_timeout.is_some_and(|timeout| timeout.is_zero()) {
        stop_handle.stop();
        Err(LlmError::Timeout("LLM run exceeded its overall deadline".into()))
    } else {
        let provider_future = registration
            .provider
            .run(provider_request, run_host.clone(), stop_token.clone());
        await_provider(provider_future, provider_timeout, &stop_token, &stop_handle).await
    };

    let (mut provider_result, invalid_identity_outcome) = match provider_result {
        Ok(provider_outcome) => {
            match super::super::outcome_validation::validate_identity(&provider_outcome, &provider_id, &model_id) {
                Ok(()) => (Ok(provider_outcome), None),
                Err(error) => (Err(error), Some(provider_outcome)),
            }
        }
        Err(error) => (Err(error), None),
    };

    if let Err(error) = run_host.flush_pending_finished_events().await {
        provider_result = Err(error);
    }

    if matches!(&provider_result, Err(LlmError::Cancelled)) {
        stop_handle.stop();
    }

    run_host.finalize();

    let host_results = HostResults::new(run_host.take_child_usage().await, run_host.take_tool_records().await);

    if let Some(error @ LlmError::Timeout(_)) = provider_result.as_ref().err() {
        return Ok(failed_outcome(
            error.clone(),
            provider_id,
            model_id,
            &event_dispatcher,
            host_results,
            None,
        )
        .await);
    }

    if stop_token.is_stopped() {
        return Ok(cancelled_outcome(provider_result, provider_id, model_id, &event_dispatcher, host_results).await);
    }

    let provider_outcome = match provider_result {
        Ok(provider_outcome) => attach_host_results(provider_outcome, host_results),
        Err(error) => {
            return Ok(failed_outcome(
                error,
                provider_id,
                model_id,
                &event_dispatcher,
                host_results,
                invalid_identity_outcome,
            )
            .await);
        }
    };

    Ok(match provider_outcome {
        ProviderRunOutcome::Completed(response) if stop_token.is_stopped() => LlmRunOutcome::Cancelled(response.into()),
        ProviderRunOutcome::Completed(response) => LlmRunOutcome::Completed(response),
        ProviderRunOutcome::Cancelled(partial_response) => LlmRunOutcome::Cancelled(partial_response),
    })
}

async fn await_provider(
    provider_future: crate::BoxFuture<'_, Result<ProviderRunOutcome, LlmError>>,
    overall_timeout: Option<Duration>,
    stop_token: &crate::StopToken,
    stop_handle: &crate::StopHandle,
) -> Result<ProviderRunOutcome, LlmError> {
    tokio::pin!(provider_future);

    match overall_timeout {
        Some(timeout) => tokio::select! {
            biased;
            () = stop_token.cancelled() => Err(LlmError::Cancelled),
            () = tokio::time::sleep(timeout) => {
            stop_handle.stop();
            Err(LlmError::Timeout("LLM run exceeded its overall deadline".into()))
            },
            provider_result = &mut provider_future => provider_result,
        },
        None => tokio::select! {
            biased;
            () = stop_token.cancelled() => Err(LlmError::Cancelled),
            provider_result = &mut provider_future => provider_result,
        },
    }
}
