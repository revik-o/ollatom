use crate::{
    BoxFuture, ConversationMessage, LlmOptions, ModelId, ProviderId, ReasoningEffort, RunPolicy,
    StopToken, Usage,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SubagentProfileId(String);

impl SubagentProfileId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, crate::LlmError> {
        let value = value.as_ref().trim();

        if !crate::ids::is_valid_identifier(value) {
            return Err(crate::LlmError::InvalidRequest(format!(
                "invalid subagent profile ID: {value}"
            )));
        }

        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SubagentProfileId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone)]
pub struct SubagentProfile {
    pub provider: ProviderId,
    pub model: Option<ModelId>,
    pub effort: ReasoningEffort,
    pub system_prompt: Option<String>,
    pub tool_names: Vec<String>,
    pub policy: RunPolicy,
    pub max_tokens: Option<u64>,
    pub timeout: Option<Duration>,
    pub allow_context: bool,
    pub max_depth: u8,
    pub options: LlmOptions,
}

#[derive(Clone, Default)]
pub struct SubagentProfileRegistry {
    profiles: BTreeMap<SubagentProfileId, SubagentProfile>,
}

impl SubagentProfileRegistry {
    pub fn register(
        &mut self,
        profile_id: SubagentProfileId,
        profile: SubagentProfile,
    ) -> Result<(), crate::LlmError> {
        let tool_names = profile.tool_names.iter().collect::<BTreeSet<_>>();
        let has_invalid_budget = profile.max_tokens == Some(0)
            || profile.timeout.is_some_and(|timeout| timeout.is_zero());
        let has_empty_tool_name = profile.tool_names.iter().any(|name| name.trim().is_empty());
        let has_duplicate_tool_names = tool_names.len() != profile.tool_names.len();
        let profile_is_invalid =
            has_invalid_budget || has_empty_tool_name || has_duplicate_tool_names;

        if profile_is_invalid {
            return Err(crate::LlmError::InvalidRequest(
                "subagent profile contains an invalid budget or tool name".into(),
            ));
        }

        if self.profiles.contains_key(&profile_id) {
            return Err(crate::LlmError::InvalidRequest(format!(
                "duplicate subagent profile {}",
                profile_id.as_str()
            )));
        }

        self.profiles.insert(profile_id, profile);

        Ok(())
    }

    pub fn get(&self, profile_id: &SubagentProfileId) -> Option<&SubagentProfile> {
        self.profiles.get(profile_id)
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InvokeSubagentRequest {
    pub task: String,
    pub profile: SubagentProfileId,
}
impl InvokeSubagentRequest {
    pub fn validate(&self) -> Result<(), crate::LlmError> {
        if self.task.trim().is_empty() {
            return Err(crate::LlmError::InvalidToolArguments(
                "subagent task cannot be empty".into(),
            ));
        }

        SubagentProfileId::new(self.profile.as_str())?;

        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubagentOutcome {
    pub text: String,
    pub usage: Usage,
}

pub trait SubagentRunner: Send + Sync {
    fn invoke(
        &self,
        request: InvokeSubagentRequest,
        profile: SubagentProfile,
        inherited_context: Vec<ConversationMessage>,
        stop: StopToken,
        remaining_depth: u8,
    ) -> BoxFuture<'static, Result<SubagentOutcome, crate::LlmError>>;
}

pub(crate) type SharedSubagentRunner = Arc<dyn SubagentRunner>;
