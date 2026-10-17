use super::{BasicToolConfiguration, FilesystemToolOperation, filesystem_listing, filesystem_operations, tool_support};
use crate::ToolFailure;
use serde_json::Value;
use std::sync::Arc;

pub(super) fn resolve_workspace_file(
    configuration: &BasicToolConfiguration,
    arguments: &Value,
    argument_name: &str,
) -> Result<filesystem::WorkspaceFile, ToolFailure> {
    tool_support::resolve_workspace_file_argument(configuration, arguments, argument_name)
}

pub(super) async fn execute_filesystem_operation(
    configuration: Arc<BasicToolConfiguration>,
    operation: FilesystemToolOperation,
    tool_arguments: &Value,
    filesystem_capability: Option<filesystem::WorkspaceCapability>,
    stop: crate::StopToken,
) -> Result<Value, ToolFailure> {
    match (operation, filesystem_capability) {
        (FilesystemToolOperation::ListFiles, Some(filesystem::WorkspaceCapability::Directory(_))) => {
            filesystem_listing::list_files(configuration, stop).await
        }
        (FilesystemToolOperation::ReadFile, Some(filesystem::WorkspaceCapability::File(file))) => {
            filesystem_operations::read_file_from_workspace(&configuration, tool_arguments, file).await
        }
        (FilesystemToolOperation::WriteFile, Some(filesystem::WorkspaceCapability::File(file))) => {
            filesystem_operations::write_file_from_workspace(&configuration, tool_arguments, file).await
        }
        (FilesystemToolOperation::CreateDirectory, Some(filesystem::WorkspaceCapability::File(file))) => {
            filesystem_operations::create_directory_from_workspace(tool_arguments, file).await
        }
        (FilesystemToolOperation::RenamePath, Some(filesystem::WorkspaceCapability::Rename { source, destination })) => {
            filesystem_operations::rename_path_from_workspace(tool_arguments, source, destination).await
        }
        (FilesystemToolOperation::DeletePath, Some(filesystem::WorkspaceCapability::File(file))) => {
            filesystem_operations::delete_path_from_workspace(tool_arguments, file).await
        }
        _ => Err(ToolFailure::Execution(
            "filesystem capability missing from tool plan".into(),
        )),
    }
}
