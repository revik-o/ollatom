use llm::{
    CapabilitySupport, LocalOptionPhase, ModelCapability, ProviderCapabilities, ReasoningEffort,
};
use std::collections::BTreeMap;

pub(crate) fn provider_capabilities() -> ProviderCapabilities {
    let features = BTreeMap::from([
        (ModelCapability::ChatCompletion, CapabilitySupport::Unknown),
        (ModelCapability::Streaming, CapabilitySupport::Supported),
        (ModelCapability::Tools, CapabilitySupport::Unknown),
        (ModelCapability::Images, CapabilitySupport::Unknown),
        (ModelCapability::Audio, CapabilitySupport::Unknown),
        (ModelCapability::Reasoning, CapabilitySupport::Unknown),
        (ModelCapability::Files, CapabilitySupport::Unsupported),
        (
            ModelCapability::StructuredOutput,
            CapabilitySupport::Unsupported,
        ),
    ]);
    let reasoning_efforts = BTreeMap::from([
        (ReasoningEffort::Auto, CapabilitySupport::Supported),
        (ReasoningEffort::None, CapabilitySupport::Supported),
        (ReasoningEffort::Minimal, CapabilitySupport::Unsupported),
        (ReasoningEffort::Low, CapabilitySupport::Unknown),
        (ReasoningEffort::Medium, CapabilitySupport::Unknown),
        (ReasoningEffort::High, CapabilitySupport::Unknown),
        (ReasoningEffort::ExtraHigh, CapabilitySupport::Unsupported),
        (ReasoningEffort::Max, CapabilitySupport::Unknown),
    ]);
    let generation_options = supported_options([
        "max_output_tokens",
        "temperature",
        "diversity_threshold",
        "max_token_choices",
        "repeat_penalty",
        "stop_sequences",
    ]);
    let local_runtime_options = supported_options([
        "context_size",
        "evaluation_batch_size",
        "threads",
        "keep_alive_seconds",
    ]);
    let local_option_phases = [
        "context_size",
        "evaluation_batch_size",
        "threads",
        "keep_alive_seconds",
    ]
    .into_iter()
    .map(|name| (name.to_string(), LocalOptionPhase::PerRequest))
    .collect();

    ProviderCapabilities {
        features,
        reasoning_efforts,
        generation_options,
        local_runtime_options,
        local_option_phases,
    }
}

fn supported_options<const COUNT: usize>(
    names: [&str; COUNT],
) -> BTreeMap<String, CapabilitySupport> {
    names
        .into_iter()
        .map(|name| (name.to_string(), CapabilitySupport::Supported))
        .collect()
}
