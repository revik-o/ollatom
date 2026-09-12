use super::metadata::{NativeModelTag, NativeRunningModel};
use llm::{CapabilitySupport, ModelCapability, ModelScope, ProviderCapabilities, ReasoningEffort};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn model_info_value<'a>(model_info: &'a Value, suffix: &str) -> Option<&'a Value> {
    model_info.as_object()?.iter().find_map(|(key, value)| {
        (key == suffix || key.ends_with(&format!(".{suffix}"))).then_some(value)
    })
}

pub(super) fn derive_owner_from_model_id(model_id: &str) -> Option<String> {
    let mut components = model_id.split('/');
    let first_component = components.next()?;

    if first_component == "hf.co" {
        components.next().map(ToString::to_string)
    } else if model_id.contains('/') {
        Some(first_component.to_string())
    } else {
        None
    }
}

pub(super) fn native_model_is_remote(
    model_tag: Option<&NativeModelTag>,
    running_model: Option<&NativeRunningModel>,
) -> bool {
    [
        model_tag.map(NativeModelTag::raw_value),
        running_model.map(NativeRunningModel::raw_value),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        value.as_object().is_some_and(|object| {
            [
                "remote",
                "remote_model",
                "remote_host",
                "remote_url",
                "cloud",
            ]
            .iter()
            .any(|key| {
                object.get(*key).is_some_and(|entry| match entry {
                    Value::Bool(value) => *value,
                    Value::String(value) => !value.is_empty(),
                    Value::Null => false,
                    _ => true,
                })
            })
        })
    })
}

pub(super) fn model_matches_scope(scope: ModelScope, is_remote: bool, is_loaded: bool) -> bool {
    match scope {
        ModelScope::All => true,
        ModelScope::Local => !is_remote,
        ModelScope::Remote => is_remote,
        ModelScope::Loaded => is_loaded,
    }
}

pub(super) fn first_unsigned_integer<'a>(
    values: impl IntoIterator<Item = Option<&'a Value>>,
) -> Option<u64> {
    values
        .into_iter()
        .flatten()
        .find_map(parse_unsigned_integer)
}

pub(super) fn parse_unsigned_integer(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|string| string.parse().ok()))
}

pub(super) fn parse_parameter_value(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(parse_parameter_count))
}

pub(super) fn first_string<'a>(
    values: impl IntoIterator<Item = Option<&'a Value>>,
) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .find_map(|value| value.as_str().map(ToString::to_string))
}

fn parse_parameter_count(value: &str) -> Option<u64> {
    let normalized = value.trim().to_ascii_uppercase().replace(',', "");
    let (numeric_text, multiplier) = match normalized.chars().last() {
        Some('T') => (&normalized[..normalized.len() - 1], 1_000_000_000_000_u128),
        Some('B') => (&normalized[..normalized.len() - 1], 1_000_000_000_u128),
        Some('M') => (&normalized[..normalized.len() - 1], 1_000_000_u128),
        Some('K') => (&normalized[..normalized.len() - 1], 1_000_u128),
        Some(_) => (normalized.as_str(), 1_u128),
        None => return None,
    };

    parse_scaled_parameter_count(numeric_text.trim(), multiplier)
}

fn parse_scaled_parameter_count(numeric_text: &str, multiplier: u128) -> Option<u64> {
    let mut decimal_parts = numeric_text.split('.');
    let integer_text = decimal_parts.next()?;
    let fractional_text = decimal_parts.next();

    if decimal_parts.next().is_some()
        || integer_text.is_empty() && fractional_text.is_none_or(str::is_empty)
    {
        return None;
    }

    let integer_value = if integer_text.is_empty() {
        0
    } else {
        integer_text.parse::<u128>().ok()?
    };
    let scaled_integer = integer_value.checked_mul(multiplier)?;
    let scaled_fraction = match fractional_text {
        Some("") | None => 0,
        Some(fractional_text) => {
            let fractional_value = fractional_text.parse::<u128>().ok()?;
            let fractional_digit_count = u32::try_from(fractional_text.len()).ok()?;
            let fractional_scale = 10_u128.checked_pow(fractional_digit_count)?;
            fractional_value
                .checked_mul(multiplier)?
                .checked_add(fractional_scale / 2)?
                / fractional_scale
        }
    };

    u64::try_from(scaled_integer.checked_add(scaled_fraction)?).ok()
}

pub(super) fn native_capabilities(
    show: &Value,
    tag: Option<&Value>,
    running: Option<&Value>,
) -> Vec<String> {
    let values = show
        .get("capabilities")
        .or_else(|| tag.and_then(|value| value.get("capabilities")))
        .or_else(|| running.and_then(|value| value.get("capabilities")))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();

    values.into_iter().collect()
}

pub(super) fn apply_model_capabilities(capabilities: &mut ProviderCapabilities, native: &[String]) {
    let native = native
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    if native.contains("tools") {
        capabilities
            .features
            .insert(ModelCapability::Tools, CapabilitySupport::Supported);
    }

    if native.contains("vision") {
        capabilities
            .features
            .insert(ModelCapability::Images, CapabilitySupport::Supported);
    }

    if native.contains("audio") {
        capabilities
            .features
            .insert(ModelCapability::Audio, CapabilitySupport::Supported);
    }

    if native.contains("thinking") {
        capabilities
            .features
            .insert(ModelCapability::Reasoning, CapabilitySupport::Supported);

        for effort in [
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::Max,
        ] {
            capabilities
                .reasoning_efforts
                .insert(effort, CapabilitySupport::Supported);
        }
    }
}
