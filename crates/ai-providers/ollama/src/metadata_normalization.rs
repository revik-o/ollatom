use super::{
    metadata::ModelInformationSources,
    metadata_values::{first_string, first_unsigned_integer, model_info_value},
};
use llm::{ModelId, ModelState, NormalizedValue, ValueProvenance};

pub(super) fn api_reported_value<ValueType>(value: ValueType) -> NormalizedValue<ValueType> {
    NormalizedValue {
        value: Some(value),
        provenance: ValueProvenance::ApiReported,
    }
}

pub(super) fn inferred_value<ValueType>(value: ValueType) -> NormalizedValue<ValueType> {
    NormalizedValue {
        value: Some(value),
        provenance: ValueProvenance::Inferred,
    }
}

pub(super) fn optional_api_reported_value<ValueType>(
    value: Option<ValueType>,
) -> NormalizedValue<ValueType> {
    match value {
        Some(value) => api_reported_value(value),
        None => unknown_value(),
    }
}

pub(super) fn unknown_value<ValueType>() -> NormalizedValue<ValueType> {
    NormalizedValue {
        value: None,
        provenance: ValueProvenance::Unknown,
    }
}

pub(super) fn normalized_display_name(
    model_identifier: &ModelId,
    sources: &ModelInformationSources<'_>,
) -> NormalizedValue<String> {
    let reported_display_name = first_string([
        sources.model_details.get("name"),
        sources.model_tag.and_then(|value| value.get("name")),
    ]);

    match reported_display_name {
        Some(display_name) => api_reported_value(display_name),
        None => inferred_value(model_identifier.as_str().to_string()),
    }
}

pub(super) fn normalized_context_window(
    sources: &ModelInformationSources<'_>,
) -> NormalizedValue<u32> {
    let context_window = first_unsigned_integer([
        sources
            .architecture_information
            .and_then(|value| value.get("context_length")),
        sources.model_details.get("context_length"),
        sources
            .model_tag
            .and_then(|value| value.get("context_length")),
    ])
    .or_else(|| {
        sources
            .architecture_information
            .and_then(|value| model_info_value(value, "context_length"))
            .and_then(super::metadata_values::parse_unsigned_integer)
    })
    .and_then(|value| u32::try_from(value).ok());

    optional_api_reported_value(context_window)
}

pub(super) fn normalized_maximum_output_tokens(
    sources: &ModelInformationSources<'_>,
) -> NormalizedValue<u32> {
    let maximum_output_tokens = first_unsigned_integer([
        sources
            .architecture_information
            .and_then(|value| value.get("max_output_tokens")),
        sources.model_details.get("max_output_tokens"),
        sources
            .details
            .and_then(|value| value.get("max_output_tokens")),
    ])
    .or_else(|| {
        sources
            .architecture_information
            .and_then(|value| model_info_value(value, "max_output_tokens"))
            .and_then(super::metadata_values::parse_unsigned_integer)
    })
    .and_then(|value| u32::try_from(value).ok());

    optional_api_reported_value(maximum_output_tokens)
}

pub(super) fn normalized_quantization(
    sources: &ModelInformationSources<'_>,
) -> NormalizedValue<String> {
    optional_api_reported_value(first_string([
        sources
            .details
            .and_then(|value| value.get("quantization_level")),
        sources.details.and_then(|value| value.get("quantization")),
        sources
            .model_tag
            .and_then(|value| value.get("quantization_level")),
    ]))
}

pub(super) fn normalized_owner(
    model_identifier: &ModelId,
    sources: &ModelInformationSources<'_>,
) -> NormalizedValue<String> {
    let reported_owner = first_string([
        sources.model_details.get("owner"),
        sources.details.and_then(|value| value.get("owner")),
        sources.details.and_then(|value| value.get("family")),
        sources
            .details
            .and_then(|value| value.get("families").and_then(|families| families.get(0))),
    ]);

    match reported_owner {
        Some(owner) => api_reported_value(owner),
        None => match super::metadata_values::derive_owner_from_model_id(model_identifier.as_str())
        {
            Some(owner) => inferred_value(owner),
            None => unknown_value(),
        },
    }
}

pub(super) fn normalized_model_state(
    has_model_tag: bool,
    has_model_details: bool,
    is_running: bool,
) -> NormalizedValue<ModelState> {
    if is_running {
        api_reported_value(ModelState::Available)
    } else if has_model_tag || has_model_details {
        inferred_value(ModelState::Unloaded)
    } else {
        inferred_value(ModelState::Missing)
    }
}
