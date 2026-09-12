use super::{
    content_media::infer_media_type,
    metadata_values::{first_unsigned_integer, model_info_value},
};
use llm::{
    ContentBlock, ContextOverflowPolicy, ConversationMessage, LlmError, LocalOptionPhase, ModelId,
    OptionHandlingMode, ProviderRunRequest, ReasoningEffort, ToolOutput,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

const CONTEXT_OPTIMIZATION_PRESSURE_NUMERATOR: u64 = 9;
const CONTEXT_OPTIMIZATION_PRESSURE_DENOMINATOR: u64 = 10;

pub(super) fn estimate_history_tokens(history: &[ConversationMessage]) -> u64 {
    serde_json::to_vec(history).map_or(u64::MAX, |serialized_history| {
        let byte_count = u64::try_from(serialized_history.len()).unwrap_or(u64::MAX);
        byte_count.saturating_add(3) / 4
    })
}

pub(super) fn estimate_new_tool_data_tokens(outputs: &[ToolOutput]) -> u64 {
    serde_json::to_vec(outputs).map_or(u64::MAX, |serialized_outputs| {
        let byte_count = u64::try_from(serialized_outputs.len()).unwrap_or(u64::MAX);
        byte_count.saturating_add(3) / 4
    })
}

pub(super) fn context_input_limit(
    request: &ProviderRunRequest,
    model_details: &Value,
) -> Option<u64> {
    [request.options.local.context_size.map(u64::from)]
        .into_iter()
        .flatten()
        .filter(|value| *value > 0)
        .min()
        .or_else(|| {
            first_unsigned_integer([
                model_details.get("context_length"),
                model_details
                    .get("model_info")
                    .and_then(|value| value.get("context_length")),
                model_details
                    .get("model_info")
                    .and_then(|value| model_info_value(value, "context_length")),
            ])
        })
}

pub(super) fn context_pressure_reached(
    observed_input_tokens: u64,
    estimated_tool_tokens: u64,
    limit: u64,
) -> bool {
    let pressure = observed_input_tokens.saturating_add(estimated_tool_tokens);
    pressure.saturating_mul(CONTEXT_OPTIMIZATION_PRESSURE_DENOMINATOR)
        >= limit.saturating_mul(CONTEXT_OPTIMIZATION_PRESSURE_NUMERATOR)
}

pub(super) fn model_is_gemma_four(model: &ModelId) -> bool {
    model.as_str().to_ascii_lowercase().starts_with("gemma4")
}

pub(super) fn build_options(
    request: &ProviderRunRequest,
) -> Result<(Map<String, Value>, Option<Value>), LlmError> {
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

    if options.context.input_token_budget.is_some()
        && options.handling == OptionHandlingMode::Strict
    {
        return Err(LlmError::UnsupportedOption("input_token_budget".into()));
    }

    configure_native_context_overflow(request, &mut native_options)?;

    let native_reasoning = match options.reasoning.effort {
        ReasoningEffort::Auto => None,
        ReasoningEffort::None => Some(json!(false)),
        ReasoningEffort::Minimal | ReasoningEffort::ExtraHigh => {
            return Err(LlmError::UnsupportedEffort(format!(
                "{:?}",
                options.reasoning.effort
            )));
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
    let native_truncation = if context_options.optimization {
        Some(false)
    } else {
        match context_options.overflow_policy {
            Some(ContextOverflowPolicy::Error) => Some(false),
            Some(ContextOverflowPolicy::TruncateOldest) => Some(true),
            Some(ContextOverflowPolicy::Summarize)
                if request.options.handling == OptionHandlingMode::Strict =>
            {
                return Err(LlmError::UnsupportedOption(
                    "context_overflow:summarize".into(),
                ));
            }
            Some(ContextOverflowPolicy::Summarize) | None => None,
        }
    };

    if let Some(native_truncation) = native_truncation {
        native_options.insert("truncate".into(), json!(native_truncation));
        native_options.insert("shift".into(), json!(native_truncation));
    }

    Ok(())
}

pub(super) fn validate_model_features(
    model_details: &Value,
    request: &ProviderRunRequest,
) -> Result<(), LlmError> {
    let capabilities = model_details
        .get("capabilities")
        .and_then(Value::as_array)
        .map_or_else(BTreeSet::new, |values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_ascii_lowercase)
                .collect::<BTreeSet<_>>()
        });

    if !request.tools.is_empty() && !capabilities.contains("tools") {
        return Err(LlmError::UnsupportedOption("tools".into()));
    }

    let has_image = request_contains_media(request, "image/");
    let has_audio = request_contains_media(request, "audio/");

    if has_image && !capabilities.contains("vision") {
        return Err(LlmError::UnsupportedOption("images".into()));
    }

    if has_audio && !capabilities.contains("audio") {
        return Err(LlmError::UnsupportedOption("audio".into()));
    }

    if !matches!(
        request.options.reasoning.effort,
        ReasoningEffort::Auto | ReasoningEffort::None
    ) && !capabilities.contains("thinking")
    {
        return Err(LlmError::UnsupportedEffort(format!(
            "{:?}",
            request.options.reasoning.effort
        )));
    }

    Ok(())
}

fn request_contains_media(request: &ProviderRunRequest, prefix: &str) -> bool {
    request
        .context
        .iter()
        .flat_map(|message| message.content.iter())
        .chain(request.user_message.content.iter())
        .any(|block| match block {
            ContentBlock::Binary { media_type, .. } => media_type.starts_with(prefix),
            ContentBlock::File { media_type, path } => media_type.as_deref().map_or_else(
                || infer_media_type(path, None).is_ok_and(|value| value.starts_with(prefix)),
                |value| value.starts_with(prefix),
            ),
            _ => false,
        })
}
