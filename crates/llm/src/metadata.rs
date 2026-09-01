use crate::{LocalOptionPhase, ModelId, ProviderId, ReasoningEffort};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueProvenance {
    ApiReported,
    StaticCatalog,
    Inferred,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizedValue<T> {
    pub value: Option<T>,
    pub provenance: ValueProvenance,
}

impl<T> Default for NormalizedValue<T> {
    fn default() -> Self {
        Self {
            value: None,
            provenance: ValueProvenance::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Streaming,
    Tools,
    Images,
    Audio,
    Files,
    Reasoning,
    StructuredOutput,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub features: BTreeMap<ModelCapability, CapabilitySupport>,
    pub reasoning_efforts: BTreeMap<ReasoningEffort, CapabilitySupport>,
    pub generation_options: BTreeMap<String, CapabilitySupport>,
    pub local_runtime_options: BTreeMap<String, CapabilitySupport>,
    pub local_option_phases: BTreeMap<String, LocalOptionPhase>,
}

impl ProviderCapabilities {
    pub fn support_for(&self, capability: ModelCapability) -> CapabilitySupport {
        self.features.get(&capability).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    Available,
    Loading,
    Unloaded,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub provider: ProviderId,
    pub id: ModelId,
    pub display_name: NormalizedValue<String>,
    pub context_window: NormalizedValue<u32>,
    pub max_output_tokens: NormalizedValue<u32>,
    pub state: NormalizedValue<ModelState>,
    pub quantization: NormalizedValue<String>,
    pub owner: NormalizedValue<String>,
    pub capabilities: ProviderCapabilities,
    #[serde(default)]
    pub provider_metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelScope {
    #[default]
    All,
    Local,
    Remote,
    Loaded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityState {
    Ready,
    MissingConfiguration,
    Unreachable,
    Unauthorized,
    ModelMissing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvailabilityReport {
    pub provider: ProviderId,
    pub state: AvailabilityState,
    pub endpoint: AvailabilityState,
    pub authentication: AvailabilityState,
    pub model: AvailabilityState,
    pub selected_model: Option<ModelId>,
    pub message: Option<String>,
}

impl AvailabilityReport {
    pub fn is_ready(&self) -> bool {
        self.state == AvailabilityState::Ready
    }
}
