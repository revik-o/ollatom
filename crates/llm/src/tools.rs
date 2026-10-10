use crate::{BoxFuture, LlmError, RequiredCapability, StopToken};
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod registry;

pub use registry::ToolRegistry;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAuthorizationRequirement {
    CapabilityPolicy,
    ExplicitUserApproval,
}

impl ToolAuthorizationRequirement {
    pub const fn requires_explicit_user_approval(self) -> bool {
        matches!(self, Self::ExplicitUserApproval)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub authorization_requirement: ToolAuthorizationRequirement,
}

impl ToolDefinition {
    pub fn validate(&self) -> Result<(), LlmError> {
        self.compile_input_schema().map(|_| ())
    }

    fn compile_input_schema(&self) -> Result<jsonschema::Validator, LlmError> {
        if !crate::ids::is_valid_identifier(&self.name) {
            return Err(LlmError::InvalidToolDefinition(self.name.clone()));
        }

        if !self.input_schema.is_object() {
            return Err(LlmError::InvalidToolDefinition(format!(
                "{} schema must be an object",
                self.name
            )));
        }

        crate::schema::validate_schema_contract(&self.input_schema).map_err(|message| {
            LlmError::InvalidToolDefinition(format!("{} schema: {message}", self.name))
        })?;

        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&self.input_schema)
            .map_err(|error| {
                LlmError::InvalidToolDefinition(format!(
                    "{} schema could not be compiled: {error}",
                    self.name
                ))
            })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolOutput {
    Success {
        call_id: String,
        content: serde_json::Value,
    },
    Failure {
        call_id: String,
        error: ToolFailure,
    },
}

impl ToolOutput {
    pub fn success(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self::Success {
            call_id: call_id.into(),
            content,
        }
    }

    pub fn failure(call_id: impl Into<String>, failure: ToolFailure) -> Self {
        Self::Failure {
            call_id: call_id.into(),
            error: failure,
        }
    }

    #[must_use]
    pub fn call_id(&self) -> &str {
        match self {
            Self::Success { call_id, .. } | Self::Failure { call_id, .. } => call_id,
        }
    }

    #[must_use]
    pub fn content(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Success { content, .. } => Some(content),
            Self::Failure { .. } => None,
        }
    }

    #[must_use]
    pub fn as_failure(&self) -> Option<&ToolFailure> {
        match self {
            Self::Success { .. } => None,
            Self::Failure { error, .. } => Some(error),
        }
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    #[must_use]
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum ToolFailure {
    InvalidArguments(String),
    Denied(String),
    Execution(String),
    Timeout(String),
}

#[derive(Clone, Debug)]
pub struct ToolPlan {
    pub call: ToolCall,
    pub normalized_arguments: serde_json::Value,
    pub required_capabilities: Vec<RequiredCapability>,
    pub timeout: Option<Duration>,
    pub filesystem_capability: Option<filesystem::WorkspaceCapability>,
}

pub struct AuthorizationGrant {
    stop: StopToken,
    authorized_capabilities: Vec<RequiredCapability>,
}

impl AuthorizationGrant {
    pub fn stop_token(&self) -> &StopToken {
        &self.stop
    }

    pub fn claim_capabilities(
        &self,
        requested_capabilities: &[RequiredCapability],
    ) -> Result<(), ToolFailure> {
        if requested_capabilities
            .iter()
            .all(|requested_capability| self.authorized_capabilities.contains(requested_capability))
        {
            Ok(())
        } else {
            Err(ToolFailure::Denied(
                "tool requested a capability that was not authorized".into(),
            ))
        }
    }

    pub(crate) fn issue(stop: StopToken, authorized_capabilities: Vec<RequiredCapability>) -> Self {
        Self {
            stop,
            authorized_capabilities,
        }
    }
}

pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure>;

    fn execute(
        &self,
        plan: ToolPlan,
        grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>>;
}

pub(crate) fn validate_arguments(
    schema: &serde_json::Value,
    arguments: &serde_json::Value,
) -> Result<(), ToolFailure> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(schema)
        .map_err(|error| ToolFailure::InvalidArguments(format!("invalid tool schema: {error}")))?;
    validator
        .validate(arguments)
        .map_err(|error| ToolFailure::InvalidArguments(error.to_string()))
}
