use crate::{ChatId, ProjectId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Chat {
    pub id: ChatId,
    pub project_id: ProjectId,
    pub name: String,
    pub llm_thinking_enabled: bool,
    pub llm_context_optimization_enabled: bool,
    pub cpu_usage_percentage: u8,
    pub gpu_usage_percentage: u8,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatInitializationParameters {
    pub llm_thinking_enabled: bool,
    pub llm_context_optimization_enabled: bool,
    pub cpu_usage_percentage: u8,
    pub gpu_usage_percentage: u8,
}

impl Default for ChatInitializationParameters {
    fn default() -> Self {
        Self {
            llm_thinking_enabled: false,
            llm_context_optimization_enabled: false,
            cpu_usage_percentage: 100,
            gpu_usage_percentage: 100,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatUpdateOptions {
    pub(crate) chat_id: ChatId,
    pub(crate) name: Option<String>,
    pub(crate) llm_thinking_enabled: Option<bool>,
    pub(crate) llm_context_optimization_enabled: Option<bool>,
    pub(crate) cpu_usage_percentage: Option<u8>,
    pub(crate) gpu_usage_percentage: Option<u8>,
}

impl ChatUpdateOptions {
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
            name: None,
            llm_thinking_enabled: None,
            llm_context_optimization_enabled: None,
            cpu_usage_percentage: None,
            gpu_usage_percentage: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_llm_thinking_enabled(mut self, is_enabled: bool) -> Self {
        self.llm_thinking_enabled = Some(is_enabled);
        self
    }

    pub fn with_llm_context_optimization_enabled(mut self, is_enabled: bool) -> Self {
        self.llm_context_optimization_enabled = Some(is_enabled);
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
        self.name.is_some()
            || self.llm_thinking_enabled.is_some()
            || self.llm_context_optimization_enabled.is_some()
            || self.cpu_usage_percentage.is_some()
            || self.gpu_usage_percentage.is_some()
    }
}
