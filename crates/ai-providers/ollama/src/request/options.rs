use llm::{ContextOverflowPolicy, LlmError, LocalOptionPhase, OptionHandlingMode, ProviderRunRequest, ReasoningEffort};
use serde_json::{Map, Value, json};

pub(crate) fn build_options(request: &ProviderRunRequest) -> Result<(Map<String, Value>, Option<Value>), LlmError> {
    let options = &request.options;
    let mut native_options = Map::new();

    if let Some(value) = options.generation.max_output_tokens {
        native_options.insert("num_predict".into(), json!(value));
    }

    if let Some(value) = options.generation.temperature {
        native_options.insert("temperature".into(), json!(value));
    }

    if let Some(value) = options.generation.diversity_threshold {
        native_options.insert("top_p".into(), json!(value));
    }

    if let Some(value) = options.generation.max_token_choices {
        native_options.insert("top_k".into(), json!(value));
    }

    if let Some(value) = options.generation.repeat_penalty {
        native_options.insert("repeat_penalty".into(), json!(value));
    }

    if let Some(value) = &options.generation.stop_sequences {
        native_options.insert("stop".into(), json!(value));
    }

    if let Some(value) = options.local.context_size {
        native_options.insert("num_ctx".into(), json!(value));
    }

    if let Some(value) = options.local.evaluation_batch_size {
        native_options.insert("num_batch".into(), json!(value));
    }

    if let Some(value) = options.local.threads {
        native_options.insert("num_thread".into(), json!(value));
    }

    if let Some(required_phase) = options.local.required_phase
        && required_phase != LocalOptionPhase::PerRequest
        && options.handling == OptionHandlingMode::Strict
    {
        return Err(LlmError::UnsupportedOption(format!(
            "required_phase:{required_phase:?}"
        )));
    }

    if options.context.input_token_budget.is_some() && options.handling == OptionHandlingMode::Strict {
        return Err(LlmError::UnsupportedOption("input_token_budget".into()));
    }

    configure_native_context_overflow(request, &mut native_options)?;

    let native_reasoning = match options.reasoning.effort {
        ReasoningEffort::Auto => None,
        ReasoningEffort::None => Some(json!(false)),
        ReasoningEffort::Minimal | ReasoningEffort::ExtraHigh => {
            return Err(LlmError::UnsupportedEffort(format!("{:?}", options.reasoning.effort)));
        }
        ReasoningEffort::Low => Some(json!("low")),
        ReasoningEffort::Medium => Some(json!("medium")),
        ReasoningEffort::High => Some(json!("high")),
        ReasoningEffort::Max => Some(json!("max")),
    };

    Ok((native_options, native_reasoning))
}

fn configure_native_context_overflow(
    request: &ProviderRunRequest,
    native_options: &mut Map<String, Value>,
) -> Result<(), LlmError> {
    let context_options = &request.options.context;
    let native_truncation = match context_options.overflow_policy {
        Some(ContextOverflowPolicy::TruncateOldest) => Some(true),
        Some(ContextOverflowPolicy::Error) => Some(false),
        Some(ContextOverflowPolicy::Summarize) if request.options.handling == OptionHandlingMode::Strict => {
            return Err(LlmError::UnsupportedOption("context_overflow:summarize".into()));
        }
        Some(ContextOverflowPolicy::Summarize) => None,
        None if context_options.optimization => Some(false),
        None => None,
    };

    if let Some(native_truncation) = native_truncation {
        native_options.insert("truncate".into(), json!(native_truncation));
        native_options.insert("shift".into(), json!(native_truncation));
    }

    Ok(())
}
