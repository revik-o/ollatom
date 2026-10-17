use crate::ProjectId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

pub type ProjectInitializationParameters = super::ExecutionSettings;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProjectFilesystemIdentity {
    Unix {
        device_identifier: u64,
        inode_identifier: u64,
    },
    Windows {
        volume_identifier: Vec<u8>,
        file_identifier: [u8; 16],
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub canonical_directory_path: PathBuf,
    pub filesystem_identity: ProjectFilesystemIdentity,
    pub execution_settings: super::ExecutionSettings,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<&Project> for ProjectId {
    fn from(project: &Project) -> Self {
        project.id
    }
}

#[must_use]
#[derive(Clone, Debug)]
pub struct ProjectUpdateOptions {
    pub(crate) project_id: ProjectId,
    pub(crate) name: Option<String>,
    pub(crate) canonical_directory_path: Option<PathBuf>,
    pub(crate) execution_settings: super::ExecutionSettingsUpdate,
}

impl ProjectUpdateOptions {
    pub fn new(project_id: impl Into<ProjectId>) -> Self {
        Self {
            project_id: project_id.into(),
            name: None,
            canonical_directory_path: None,
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

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.canonical_directory_path = Some(path.into());
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
        self.name.is_some() || self.canonical_directory_path.is_some() || self.execution_settings.contains_changes()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatabaseSchemaVersion(pub i64);
