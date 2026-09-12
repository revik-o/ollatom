use super::{
    BasicToolConfiguration, command_tool::CommandExecutionTool, filesystem_tools::FilesystemTool,
    web_tool::WebSearchTool,
};
use crate::{LlmError, ToolRegistry};
use std::sync::Arc;

pub fn register_basic_tools(
    configuration: BasicToolConfiguration,
) -> Result<ToolRegistry, LlmError> {
    configuration.validate()?;

    std::fs::create_dir_all(&configuration.root_directory).map_err(|error| {
        LlmError::InvalidRequest(format!("basic tool root is unavailable: {error}"))
    })?;

    let configuration = Arc::new(configuration);
    let mut tool_registry = ToolRegistry::new();

    for filesystem_tool in FilesystemTool::create_all(configuration.clone()) {
        tool_registry.register(filesystem_tool)?;
    }

    tool_registry.register(CommandExecutionTool::new(configuration.clone()))?;
    tool_registry.register(WebSearchTool::new())?;

    Ok(tool_registry)
}
