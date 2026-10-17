use super::{
    BasicToolConfiguration, command_tool::CommandExecutionTool, filesystem_tools::FilesystemTool,
    web_tool::WebSearchTool,
};
use crate::{LlmError, ToolRegistry};
use std::sync::Arc;

pub fn register_basic_tools(mut configuration: BasicToolConfiguration) -> Result<ToolRegistry, LlmError> {
    configuration.validate()?;

    std::fs::create_dir_all(&configuration.root_directory)
        .map_err(|error| LlmError::InvalidRequest(format!("basic tool root is unavailable: {error}")))?;

    let root_directory = configuration.root_directory.clone();

    let workspace_directory = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(filesystem::WorkspaceDirectory::open(root_directory))
            .map_err(|error| error.to_string())
    })
    .join()
    .map_err(|_| LlmError::InvalidRequest("basic tool workspace initialization panicked".into()))?
    .map_err(|error| LlmError::InvalidRequest(format!("basic tool root is unavailable: {error}")))?;

    configuration.root_directory = workspace_directory.canonical_directory_path().to_owned();
    configuration.workspace_directory = Some(workspace_directory);

    let configuration = Arc::new(configuration);
    let mut tool_registry = ToolRegistry::new();

    for filesystem_tool in FilesystemTool::create_all(configuration.clone()) {
        tool_registry.register(filesystem_tool)?;
    }

    tool_registry.register(CommandExecutionTool::new(configuration.clone()))?;
    tool_registry.register(WebSearchTool::new())?;

    Ok(tool_registry)
}
