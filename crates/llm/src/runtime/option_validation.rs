use crate::{LlmError, LlmOptions};

const POSITIVE_TOKEN_BUDGET_REQUIREMENT: &str = "token budgets must be greater than zero";

pub(super) fn validate_option_values(options: &LlmOptions) -> Result<(), LlmError> {
    validate_context_options(options)?;
    validate_reasoning_options(options)?;
    validate_generation_options(options)?;
    validate_local_runtime_options(options)?;
    validate_transport_options(options)
}

fn validate_context_options(options: &LlmOptions) -> Result<(), LlmError> {
    let token_budget_lacks_overflow_policy =
        options.context.input_token_budget.is_some() && options.context.overflow_policy.is_none();
    if token_budget_lacks_overflow_policy {
        return Err(LlmError::InvalidRequest(
            "an input token budget requires an explicit overflow policy".into(),
        ));
    }

    if options.context.input_token_budget == Some(0) {
        return Err(LlmError::InvalidRequest(
            POSITIVE_TOKEN_BUDGET_REQUIREMENT.into(),
        ));
    }

    Ok(())
}

fn validate_reasoning_options(options: &LlmOptions) -> Result<(), LlmError> {
    if options.reasoning.budget_tokens == Some(0) {
        return Err(LlmError::InvalidRequest(
            POSITIVE_TOKEN_BUDGET_REQUIREMENT.into(),
        ));
    }

    Ok(())
}

fn validate_generation_options(options: &LlmOptions) -> Result<(), LlmError> {
    let has_zero_output_limit = options.generation.max_output_tokens == Some(0);
    let has_zero_candidate_limit = options.generation.top_k == Some(0);
    if has_zero_output_limit || has_zero_candidate_limit {
        return Err(LlmError::InvalidRequest(
            "token and candidate limits must be greater than zero".into(),
        ));
    }

    let has_invalid_temperature = options
        .generation
        .temperature
        .is_some_and(|temperature| !temperature.is_finite() || temperature < 0.0);
    if has_invalid_temperature {
        return Err(LlmError::InvalidRequest(
            "temperature must be finite and non-negative".into(),
        ));
    }

    let has_invalid_top_probability = options.generation.top_p.is_some_and(|top_probability| {
        !top_probability.is_finite() || !(0.0..=1.0).contains(&top_probability)
    });
    if has_invalid_top_probability {
        return Err(LlmError::InvalidRequest(
            "top_p must be between zero and one".into(),
        ));
    }

    let has_invalid_repeat_penalty = options
        .generation
        .repeat_penalty
        .is_some_and(|repeat_penalty| !repeat_penalty.is_finite() || repeat_penalty <= 0.0);
    if has_invalid_repeat_penalty {
        return Err(LlmError::InvalidRequest(
            "repeat_penalty must be finite and positive".into(),
        ));
    }

    Ok(())
}

fn validate_local_runtime_options(options: &LlmOptions) -> Result<(), LlmError> {
    let local_runtime_sizes = [
        options.local.context_size,
        options.local.evaluation_batch_size,
        options.local.threads,
    ];

    let has_zero_local_runtime_size = local_runtime_sizes
        .into_iter()
        .flatten()
        .any(|size| size == 0);
    if has_zero_local_runtime_size {
        return Err(LlmError::InvalidRequest(
            "local runtime sizes must be greater than zero".into(),
        ));
    }

    Ok(())
}

fn validate_transport_options(options: &LlmOptions) -> Result<(), LlmError> {
    let transport_timeouts = [
        options.transport.connect_timeout_ms,
        options.transport.stream_idle_timeout_ms,
        options.transport.overall_timeout_ms,
    ];

    let has_zero_transport_timeout = transport_timeouts
        .into_iter()
        .flatten()
        .any(|timeout_milliseconds| timeout_milliseconds == 0);
    if has_zero_transport_timeout {
        return Err(LlmError::InvalidRequest(
            "transport timeouts must be greater than zero".into(),
        ));
    }

    Ok(())
}
