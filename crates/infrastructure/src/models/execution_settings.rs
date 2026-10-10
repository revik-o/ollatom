use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionSettings {
    pub large_language_model_thinking_enabled: bool,
    pub context_optimization_enabled: bool,
    pub cpu_usage_percentage: u8,
    pub gpu_usage_percentage: u8,
}

impl Default for ExecutionSettings {
    fn default() -> Self {
        Self {
            large_language_model_thinking_enabled: false,
            context_optimization_enabled: false,
            cpu_usage_percentage: 100,
            gpu_usage_percentage: 100,
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecutionSettingsUpdate {
    pub large_language_model_thinking_enabled: Option<bool>,
    pub context_optimization_enabled: Option<bool>,
    pub cpu_usage_percentage: Option<u8>,
    pub gpu_usage_percentage: Option<u8>,
}

impl ExecutionSettingsUpdate {
    pub fn with_large_language_model_thinking_enabled(mut self, is_enabled: bool) -> Self {
        self.large_language_model_thinking_enabled = Some(is_enabled);
        self
    }

    pub fn with_context_optimization_enabled(mut self, is_enabled: bool) -> Self {
        self.context_optimization_enabled = Some(is_enabled);
        self
    }

    pub fn with_cpu_usage_percentage(mut self, cpu_usage_percentage: u8) -> Self {
        self.cpu_usage_percentage = Some(cpu_usage_percentage);
        self
    }

    pub fn with_gpu_usage_percentage(mut self, gpu_usage_percentage: u8) -> Self {
        self.gpu_usage_percentage = Some(gpu_usage_percentage);
        self
    }

    pub(crate) fn contains_changes(&self) -> bool {
        self.large_language_model_thinking_enabled.is_some()
            || self.context_optimization_enabled.is_some()
            || self.cpu_usage_percentage.is_some()
            || self.gpu_usage_percentage.is_some()
    }

    pub(crate) fn apply_to(&self, existing_settings: &ExecutionSettings) -> ExecutionSettings {
        ExecutionSettings {
            large_language_model_thinking_enabled: self
                .large_language_model_thinking_enabled
                .unwrap_or(existing_settings.large_language_model_thinking_enabled),
            context_optimization_enabled: self
                .context_optimization_enabled
                .unwrap_or(existing_settings.context_optimization_enabled),
            cpu_usage_percentage: self
                .cpu_usage_percentage
                .unwrap_or(existing_settings.cpu_usage_percentage),
            gpu_usage_percentage: self
                .gpu_usage_percentage
                .unwrap_or(existing_settings.gpu_usage_percentage),
        }
    }
}
