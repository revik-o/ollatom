use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    #[default]
    Auto,
    None,
    Minimal,
    Low,
    Medium,
    High,
    ExtraHigh,
    Max,
}

impl FromStr for ReasoningEffort {
    type Err = crate::LlmError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let effort_value = value.trim().to_ascii_lowercase().replace(['_', '-'], "");

        match effort_value.as_str() {
            "auto" => Ok(Self::Auto),
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "extrahigh" => Ok(Self::ExtraHigh),
            "max" => Ok(Self::Max),
            _ => Err(crate::LlmError::UnsupportedEffort(value.into())),
        }
    }
}
impl TryFrom<&str> for ReasoningEffort {
    type Error = crate::LlmError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for ReasoningEffort {
    type Error = crate::LlmError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReasoningOptions {
    pub effort: ReasoningEffort,
    pub budget_tokens: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextOverflowPolicy {
    Error,
    TruncateOldest,
    Summarize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextOptions {
    pub input_token_budget: Option<u32>,
    pub overflow_policy: Option<ContextOverflowPolicy>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationOptions {
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub repeat_penalty: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOptionPhase {
    PerRequest,
    ServerStartup,
    ModelLoad,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalRuntimeOptions {
    pub context_size: Option<u32>,
    pub evaluation_batch_size: Option<u32>,
    pub threads: Option<u32>,
    pub keep_alive_seconds: Option<u64>,
    pub required_phase: Option<LocalOptionPhase>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TransportOptions {
    pub connect_timeout_ms: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub overall_timeout_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionHandlingMode {
    #[default]
    Strict,
    BestEffort,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmOptions {
    pub context: ContextOptions,
    pub generation: GenerationOptions,
    pub reasoning: ReasoningOptions,
    pub local: LocalRuntimeOptions,
    pub transport: TransportOptions,
    pub handling: OptionHandlingMode,
}
