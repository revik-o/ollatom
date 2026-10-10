use super::{BasicToolConfiguration, filesystem_listing, filesystem_operations, tool_support};
use crate::{
    AuthorizationGrant, BoxFuture, FilesystemAccess, Tool, ToolCall, ToolDefinition, ToolFailure,
    ToolOutput, ToolPlan,
};
use serde_json::json;
use std::sync::Arc;

mod operations;

#[derive(Clone)]
pub(super) struct FilesystemTool {
    configuration: Arc<BasicToolConfiguration>,
    operation: FilesystemToolOperation,
    definition: ToolDefinition,
}

#[derive(Clone, Copy)]
enum FilesystemToolOperation {
    ListFiles,
    ReadFile,
    WriteFile,
    CreateDirectory,
    RenamePath,
    DeletePath,
}

impl FilesystemTool {
    pub(super) fn create_all(configuration: Arc<BasicToolConfiguration>) -> [Self; 6] {
        [
            Self::new(configuration.clone(), FilesystemToolOperation::ListFiles),
            Self::new(configuration.clone(), FilesystemToolOperation::ReadFile),
            Self::new(configuration.clone(), FilesystemToolOperation::WriteFile),
            Self::new(
                configuration.clone(),
                FilesystemToolOperation::CreateDirectory,
            ),
            Self::new(configuration.clone(), FilesystemToolOperation::RenamePath),
            Self::new(configuration, FilesystemToolOperation::DeletePath),
        ]
    }

    fn new(configuration: Arc<BasicToolConfiguration>, operation: FilesystemToolOperation) -> Self {
        let definition = match operation {
            FilesystemToolOperation::ListFiles => tool_support::create_tool_definition(
                "list_files",
                "List files beneath the configured working directory.",
                json!({}),
                &[],
            ),
            FilesystemToolOperation::ReadFile => tool_support::create_tool_definition(
                "read_file",
                "Read a UTF-8 file by a relative path.",
                json!({"path": {"type": "string", "minLength": 1}}),
                &["path"],
            ),
            FilesystemToolOperation::WriteFile => tool_support::create_tool_definition(
                "write_file",
                "Atomically write UTF-8 content to a relative path.",
                json!({"path": {"type": "string", "minLength": 1}, "content": {"type": "string"}}),
                &["path", "content"],
            ),
            FilesystemToolOperation::CreateDirectory => tool_support::create_tool_definition(
                "create_directory",
                "Create a directory and missing parents by a relative path.",
                json!({"path": {"type": "string", "minLength": 1}}),
                &["path"],
            ),
            FilesystemToolOperation::RenamePath => tool_support::create_tool_definition(
                "rename_path",
                "Rename a file or directory using relative paths.",
                json!({"source": {"type": "string", "minLength": 1}, "destination": {"type": "string", "minLength": 1}}),
                &["source", "destination"],
            ),
            FilesystemToolOperation::DeletePath => tool_support::create_tool_definition(
                "delete_path",
                "Delete a file or directory by a relative path.",
                json!({"path": {"type": "string", "minLength": 1}}),
                &["path"],
            ),
        };

        Self {
            configuration,
            operation,
            definition,
        }
    }
}

impl Tool for FilesystemTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn plan(&self, tool_call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        tool_support::validate_tool_call(&self.definition, tool_call)?;

        let (required_capabilities, filesystem_capability) = match self.operation {
            FilesystemToolOperation::ListFiles => (
                vec![tool_support::filesystem_capability(
                    self.configuration.root_directory().to_owned(),
                    FilesystemAccess::Read,
                )],
                Some(filesystem::WorkspaceCapability::Directory(
                    self.configuration.workspace_directory().clone(),
                )),
            ),
            FilesystemToolOperation::ReadFile => {
                let file = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?;

                (
                    vec![tool_support::filesystem_capability(
                        self.configuration
                            .root_directory()
                            .join(file.relative_path().as_path()),
                        FilesystemAccess::Read,
                    )],
                    Some(filesystem::WorkspaceCapability::File(file)),
                )
            }
            FilesystemToolOperation::WriteFile => {
                let file = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?;

                (
                    vec![tool_support::filesystem_capability(
                        self.configuration
                            .root_directory()
                            .join(file.relative_path().as_path()),
                        FilesystemAccess::Modify,
                    )],
                    Some(filesystem::WorkspaceCapability::File(file)),
                )
            }
            FilesystemToolOperation::CreateDirectory => {
                let file = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?;

                (
                    vec![tool_support::filesystem_capability(
                        self.configuration
                            .root_directory()
                            .join(file.relative_path().as_path()),
                        FilesystemAccess::Create,
                    )],
                    Some(filesystem::WorkspaceCapability::File(file)),
                )
            }
            FilesystemToolOperation::RenamePath => {
                let source = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "source",
                )?;

                let destination = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "destination",
                )?;

                (
                    vec![
                        tool_support::filesystem_capability(
                            self.configuration
                                .root_directory()
                                .join(source.relative_path().as_path()),
                            FilesystemAccess::Rename,
                        ),
                        tool_support::filesystem_capability(
                            self.configuration
                                .root_directory()
                                .join(destination.relative_path().as_path()),
                            FilesystemAccess::Create,
                        ),
                    ],
                    Some(filesystem::WorkspaceCapability::Rename {
                        source,
                        destination,
                    }),
                )
            }
            FilesystemToolOperation::DeletePath => {
                let file = operations::resolve_workspace_file(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?;

                (
                    vec![tool_support::filesystem_capability(
                        self.configuration
                            .root_directory()
                            .join(file.relative_path().as_path()),
                        FilesystemAccess::Delete,
                    )],
                    Some(filesystem::WorkspaceCapability::File(file)),
                )
            }
        };

        Ok(tool_support::create_tool_plan_with_filesystem_capability(
            tool_call,
            tool_call.arguments.clone(),
            required_capabilities,
            None,
            filesystem_capability,
        ))
    }

    fn execute(
        &self,
        tool_plan: ToolPlan,
        authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        let configuration = self.configuration.clone();
        let operation = self.operation;

        Box::pin(async move {
            authorization_grant.claim_capabilities(&tool_plan.required_capabilities)?;

            let tool_output_content = operations::execute_filesystem_operation(
                configuration,
                operation,
                &tool_plan.normalized_arguments,
                tool_plan.filesystem_capability,
                authorization_grant.stop_token().clone(),
            )
            .await?;

            Ok(ToolOutput::success(tool_plan.call.id, tool_output_content))
        })
    }
}
