use crate::{BoxFuture, LlmError, RequiredCapability, StopToken};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub requires_authorization: bool,
}

impl ToolDefinition {
    pub fn validate(&self) -> Result<(), LlmError> {
        if !crate::ids::is_valid_identifier(&self.name) {
            return Err(LlmError::InvalidToolDefinition(self.name.clone()));
        }

        if !self.input_schema.is_object() {
            return Err(LlmError::InvalidToolDefinition(format!(
                "{} schema must be an object",
                self.name
            )));
        }

        crate::schema::validate_definition(&self.input_schema, "$").map_err(|message| {
            LlmError::InvalidToolDefinition(format!("{} schema: {message}", self.name))
        })?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub call_id: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolFailure>,
}

impl ToolOutput {
    pub fn success(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            call_id: call_id.into(),
            content,
            error: None,
        }
    }

    pub fn failure(call_id: impl Into<String>, failure: ToolFailure) -> Self {
        Self {
            call_id: call_id.into(),
            content: serde_json::Value::Null,
            error: Some(failure),
        }
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
}

pub struct AuthorizationGrant {
    fingerprint: String,
    stop: StopToken,
}

impl AuthorizationGrant {
    pub fn stop_token(&self) -> &StopToken {
        &self.stop
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub(crate) fn issue(plan: &ToolPlan, stop: StopToken) -> Self {
        Self {
            fingerprint: plan_fingerprint(plan),
            stop,
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

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}
impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) -> Result<(), LlmError> {
        self.register_shared(Arc::new(tool))
    }

    pub fn register_shared(&mut self, tool: Arc<dyn Tool>) -> Result<(), LlmError> {
        let definition = tool.definition();
        definition.validate()?;

        if self.tools.contains_key(&definition.name) {
            return Err(LlmError::InvalidToolDefinition(format!(
                "duplicate tool {}",
                definition.name
            )));
        }

        self.tools.insert(definition.name.clone(), tool);

        Ok(())
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    pub fn selected_definitions(
        &self,
        names: &BTreeSet<String>,
    ) -> Result<Vec<ToolDefinition>, LlmError> {
        names
            .iter()
            .map(|name| self.definition_for_name(name))
            .collect()
    }

    pub fn definition_for_name(&self, name: &str) -> Result<ToolDefinition, LlmError> {
        self.tools
            .get(name)
            .map(|tool| tool.definition())
            .ok_or_else(|| LlmError::InvalidRequest(format!("unknown selected tool: {name}")))
    }

    pub(crate) fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }
}

pub(crate) fn validate_arguments(
    schema: &serde_json::Value,
    arguments: &serde_json::Value,
) -> Result<(), ToolFailure> {
    crate::schema::validate(schema, arguments, "$")
}

pub(crate) fn plan_fingerprint(plan: &ToolPlan) -> String {
    let normalized_arguments = serde_json::to_string(&plan.normalized_arguments)
        .expect("serializing serde_json::Value cannot fail");

    format!("{}:{}:{normalized_arguments}", plan.call.name, plan.call.id)
}
