use crate::{ChatId, ProjectId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub type ChatInitializationParameters = super::ExecutionSettings;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Chat {
    pub id: ChatId,
    pub project_id: ProjectId,
    pub name: String,
    pub execution_settings: super::ExecutionSettings,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<&Chat> for ChatId {
    fn from(chat: &Chat) -> Self {
        chat.id
    }
}

#[must_use]
#[derive(Clone, Debug)]
pub struct ChatUpdateOptions {
    pub(crate) chat_id: ChatId,
    pub(crate) name: Option<String>,
    pub(crate) execution_settings: super::ExecutionSettingsUpdate,
}

impl ChatUpdateOptions {
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
            name: None,
            execution_settings: super::ExecutionSettingsUpdate::default(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_execution_settings(mut self, execution_settings: super::ExecutionSettingsUpdate) -> Self {
        self.execution_settings = execution_settings;
        self
    }

    pub fn with_llm_thinking_enabled(mut self, is_enabled: bool) -> Self {
        self.execution_settings = self
            .execution_settings
            .with_large_language_model_thinking_enabled(is_enabled);
        self
    }

    pub fn with_llm_context_optimization_enabled(mut self, is_enabled: bool) -> Self {
        self.execution_settings = self.execution_settings.with_context_optimization_enabled(is_enabled);
        self
    }

    pub fn with_cpu_usage_percentage(mut self, cpu_usage_percentage: u8) -> Self {
        self.execution_settings = self.execution_settings.with_cpu_usage_percentage(cpu_usage_percentage);
        self
    }

    pub fn with_gpu_usage_percentage(mut self, gpu_usage_percentage: u8) -> Self {
        self.execution_settings = self.execution_settings.with_gpu_usage_percentage(gpu_usage_percentage);
        self
    }

    pub(crate) fn contains_changes(&self) -> bool {
        self.name.is_some() || self.execution_settings.contains_changes()
    }
}
