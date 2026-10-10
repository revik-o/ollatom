use super::{
    metadata_normalization::{
        normalized_context_window, normalized_display_name, normalized_maximum_output_tokens,
        normalized_model_state, normalized_owner, normalized_quantization,
    },
    metadata_values::{apply_model_capabilities, native_capabilities, parse_parameter_value},
};
use llm::{
    CapabilitySupport, ModelCapability, ModelId, ModelInfo, ProviderCapabilities, ProviderId,
    ReasoningEffort,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

mod capabilities;

pub(super) use capabilities::provider_capabilities;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct NativeModelListResponse {
    pub(super) models: Vec<NativeModelListEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct NativeModelListEntry {
    pub(super) name: String,
    #[serde(default)]
    pub(super) model: Option<String>,
    #[serde(flatten)]
    additional_fields: Value,
}

impl NativeModelListEntry {
    pub(super) fn raw_value(&self) -> Value {
        native_model_raw_value(&self.name, self.model.as_deref(), &self.additional_fields)
    }
}

fn native_model_raw_value(
    model_name: &str,
    canonical_model_name: Option<&str>,
    additional_fields: &Value,
) -> Value {
    let mut native_model = additional_fields.as_object().cloned().unwrap_or_default();

    native_model.insert("name".into(), json!(model_name));

    if let Some(canonical_model_name) = canonical_model_name {
        native_model.insert("model".into(), json!(canonical_model_name));
    }

    Value::Object(native_model)
}

pub(super) fn create_model_info(
    model_identifier: ModelId,
    model_tag: Option<&NativeModelListEntry>,
    model_details: Option<Value>,
    running_model: Option<&NativeModelListEntry>,
    provider_identifier: &ProviderId,
) -> ModelInfo {
    let model_tag_value = model_tag.map(NativeModelListEntry::raw_value);
    let running_model_value = running_model.map(NativeModelListEntry::raw_value);
    let model_details_value = model_details.unwrap_or(Value::Null);

    let sources = ModelInformationSources::new(
        &model_details_value,
        model_tag_value.as_ref(),
        running_model_value.as_ref(),
    );

    let native_model_capabilities = sources.native_capabilities();
    let mut capabilities = provider_capabilities();

    apply_model_capabilities(&mut capabilities, &native_model_capabilities);
    apply_exact_reasoning_efforts(&mut capabilities, &model_details_value);

    if sources.model_details.get("capabilities").is_some() {
        capabilities.features.insert(
            ModelCapability::ChatCompletion,
            if native_model_capabilities
                .iter()
                .any(|capability| capability.eq_ignore_ascii_case("completion"))
            {
                CapabilitySupport::Supported
            } else {
                CapabilitySupport::Unsupported
            },
        );
    }

    let parameter_count = sources
        .details
        .and_then(|value| value.get("parameter_size"))
        .and_then(parse_parameter_value);

    ModelInfo {
        provider: provider_identifier.clone(),
        id: model_identifier.clone(),
        display_name: normalized_display_name(&model_identifier, &sources),
        context_window: normalized_context_window(&sources),
        max_output_tokens: normalized_maximum_output_tokens(&sources),
        state: normalized_model_state(
            model_tag.is_some(),
            !model_details_value.is_null(),
            running_model.is_some(),
        ),
        quantization: normalized_quantization(&sources),
        owner: normalized_owner(&model_identifier, &sources),
        capabilities,
        provider_metadata: create_provider_metadata(
            model_details_value,
            model_tag_value,
            running_model_value,
            parameter_count,
            &native_model_capabilities,
        ),
    }
}

fn apply_exact_reasoning_efforts(capabilities: &mut ProviderCapabilities, model_details: &Value) {
    let Some(thinking_metadata) = model_details.get("thinking") else {
        return;
    };

    let mut supported_values = thinking_metadata
        .get("values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    if let Some(default_value) = thinking_metadata.get("default") {
        supported_values.push(default_value);
    }

    for supported_value in supported_values {
        let reasoning_effort = match supported_value {
            Value::Bool(false) => Some(ReasoningEffort::None),
            Value::Bool(true) => Some(ReasoningEffort::Auto),
            Value::String(value) => match value.to_ascii_lowercase().as_str() {
                "auto" => Some(ReasoningEffort::Auto),
                "none" | "false" => Some(ReasoningEffort::None),
                "minimal" => Some(ReasoningEffort::Minimal),
                "low" => Some(ReasoningEffort::Low),
                "medium" => Some(ReasoningEffort::Medium),
                "high" => Some(ReasoningEffort::High),
                "xhigh" | "extra_high" => Some(ReasoningEffort::ExtraHigh),
                "max" => Some(ReasoningEffort::Max),
                _ => None,
            },
            _ => None,
        };

        if let Some(reasoning_effort) = reasoning_effort {
            capabilities
                .reasoning_efforts
                .insert(reasoning_effort, CapabilitySupport::Supported);
        }
    }
}

pub(super) struct ModelInformationSources<'a> {
    pub(super) model_details: &'a Value,
    pub(super) model_tag: Option<&'a Value>,
    pub(super) running_model: Option<&'a Value>,
    pub(super) details: Option<&'a Value>,
    pub(super) architecture_information: Option<&'a Value>,
}

impl<'a> ModelInformationSources<'a> {
    pub(super) fn new(
        model_details: &'a Value,
        model_tag: Option<&'a Value>,
        running_model: Option<&'a Value>,
    ) -> Self {
        Self {
            model_details,
            model_tag,
            running_model,
            details: model_details
                .get("details")
                .or_else(|| model_tag.and_then(|value| value.get("details"))),
            architecture_information: model_details.get("model_info"),
        }
    }

    pub(super) fn native_capabilities(&self) -> Vec<String> {
        native_capabilities(self.model_details, self.model_tag, self.running_model)
    }
}

fn create_provider_metadata(
    model_details: Value,
    model_tag: Option<Value>,
    running_model: Option<Value>,
    parameter_count: Option<u64>,
    native_capabilities: &[String],
) -> BTreeMap<String, Value> {
    let mut native_metadata = Map::new();

    native_metadata.insert("show".into(), model_details);

    if let Some(model_tag) = model_tag {
        native_metadata.insert("tag".into(), model_tag);
    }

    if let Some(running_model) = running_model {
        native_metadata.insert("running".into(), running_model);
    }

    if let Some(parameter_count) = parameter_count {
        native_metadata.insert("parameter_count".into(), json!(parameter_count));
    }

    native_metadata.insert("capabilities".into(), json!(native_capabilities));

    BTreeMap::from([("ollama".into(), Value::Object(native_metadata))])
}
