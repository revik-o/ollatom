use super::{BasicToolConfiguration, filesystem_listing, filesystem_operations, tool_support};
use crate::{
    AuthorizationGrant, BoxFuture, FilesystemAccess, Tool, ToolCall, ToolDefinition, ToolFailure,
    ToolOutput, ToolPlan,
};
use serde_json::{Value, json};
use std::sync::Arc;

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

        let required_capabilities = match self.operation {
            FilesystemToolOperation::ListFiles => vec![tool_support::filesystem_capability(
                self.configuration.root_directory().to_owned(),
                FilesystemAccess::Read,
            )],
            FilesystemToolOperation::ReadFile => vec![tool_support::filesystem_capability(
                tool_support::resolve_path_argument(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?,
                FilesystemAccess::Read,
            )],
            FilesystemToolOperation::WriteFile => vec![tool_support::filesystem_capability(
                tool_support::resolve_path_argument(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?,
                FilesystemAccess::Modify,
            )],
            FilesystemToolOperation::CreateDirectory => vec![tool_support::filesystem_capability(
                tool_support::resolve_path_argument(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?,
                FilesystemAccess::Create,
            )],
            FilesystemToolOperation::RenamePath => vec![
                tool_support::filesystem_capability(
                    tool_support::resolve_path_argument(
                        &self.configuration,
                        &tool_call.arguments,
                        "source",
                    )?,
                    FilesystemAccess::Rename,
                ),
                tool_support::filesystem_capability(
                    tool_support::resolve_path_argument(
                        &self.configuration,
                        &tool_call.arguments,
                        "destination",
                    )?,
                    FilesystemAccess::Create,
                ),
            ],
            FilesystemToolOperation::DeletePath => vec![tool_support::filesystem_capability(
                tool_support::resolve_path_argument(
                    &self.configuration,
                    &tool_call.arguments,
                    "path",
                )?,
                FilesystemAccess::Delete,
            )],
        };

        Ok(tool_support::create_tool_plan(
            tool_call,
            tool_call.arguments.clone(),
            required_capabilities,
            None,
        ))
    }

    fn execute(
        &self,
        tool_plan: ToolPlan,
        _authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        let configuration = self.configuration.clone();
        let operation = self.operation;

        Box::pin(async move {
            let tool_output_content = execute_filesystem_operation(
                configuration,
                operation,
                &tool_plan.normalized_arguments,
            )
            .await?;

            Ok(ToolOutput::success(tool_plan.call.id, tool_output_content))
        })
    }
}

async fn execute_filesystem_operation(
    configuration: Arc<BasicToolConfiguration>,
    operation: FilesystemToolOperation,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    match operation {
        FilesystemToolOperation::ListFiles => filesystem_listing::list_files(configuration).await,
        FilesystemToolOperation::ReadFile => {
            filesystem_operations::read_file(&configuration, tool_arguments).await
        }
        FilesystemToolOperation::WriteFile => {
            filesystem_operations::write_file(&configuration, tool_arguments).await
        }
        FilesystemToolOperation::CreateDirectory => {
            filesystem_operations::create_directory(&configuration, tool_arguments).await
        }
        FilesystemToolOperation::RenamePath => {
            filesystem_operations::rename_path(&configuration, tool_arguments).await
        }
        FilesystemToolOperation::DeletePath => {
            filesystem_operations::delete_path(&configuration, tool_arguments).await
        }
    }
}
