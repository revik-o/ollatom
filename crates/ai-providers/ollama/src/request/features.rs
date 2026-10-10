use super::super::content_media::infer_media_type;
use llm::{ContentBlock, LlmError, ProviderRunRequest, ReasoningEffort};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn validate_model_features(
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

    if model_details.get("capabilities").is_some() && !capabilities.contains("completion") {
        return Err(LlmError::UnsupportedOption("chat_completion".into()));
    }

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
