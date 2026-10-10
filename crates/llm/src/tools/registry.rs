use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct RegisteredTool {
    definition: ToolDefinition,
    compiled_input_schema: Arc<jsonschema::Validator>,
    implementation: Arc<dyn Tool>,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
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
        let compiled_input_schema = definition.compile_input_schema()?;

        if self.tools.contains_key(&definition.name) {
            return Err(LlmError::InvalidToolDefinition(format!(
                "duplicate tool {}",
                definition.name
            )));
        }

        self.tools.insert(
            definition.name.clone(),
            RegisteredTool {
                definition,
                compiled_input_schema: Arc::new(compiled_input_schema),
                implementation: tool,
            },
        );

        Ok(())
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| tool.definition.clone())
            .collect()
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
            .map(|tool| tool.definition.clone())
            .ok_or_else(|| LlmError::InvalidRequest(format!("unknown selected tool: {name}")))
    }

    pub(crate) fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .get(name)
            .map(|registered_tool| registered_tool.implementation.clone())
    }

    pub(crate) fn validate_arguments(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<(), ToolFailure> {
        let registered_tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolFailure::InvalidArguments(format!("unknown tool {name}")))?;

        registered_tool
            .compiled_input_schema
            .validate(arguments)
            .map_err(|error| ToolFailure::InvalidArguments(error.to_string()))
    }
}
