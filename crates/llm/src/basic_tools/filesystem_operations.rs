use super::BasicToolConfiguration;
use crate::ToolFailure;
use filesystem::{AtomicCommitStatus, AtomicMetadataPolicy};
use serde_json::{Value, json};

pub(super) async fn read_file_from_workspace(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
    workspace_file: filesystem::WorkspaceFile,
) -> Result<Value, ToolFailure> {
    let file_content = workspace_file
        .read_regular_file_text_bounded(configuration.maximum_file_size)
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "read_file", "path": tool_arguments["path"], "content": file_content}))
}

pub(super) async fn write_file_from_workspace(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
    workspace_file: filesystem::WorkspaceFile,
) -> Result<Value, ToolFailure> {
    let file_content = tool_arguments
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolFailure::InvalidArguments("content must be a string".into()))?;

    let content_size = u64::try_from(file_content.len()).unwrap_or(u64::MAX);

    if content_size > configuration.maximum_file_size {
        return Err(ToolFailure::InvalidArguments("content is too large".into()));
    }

    let commit = workspace_file
        .prepare_atomic_write(
            file_content.as_bytes().to_vec(),
            AtomicMetadataPolicy::PreserveSupported,
        )
        .await
        .map_err(filesystem_operation_failure)?
        .start_commit()
        .map_err(filesystem_operation_failure)?;

    match commit.wait().await {
        AtomicCommitStatus::Succeeded => {}
        AtomicCommitStatus::Failed(error) => return Err(ToolFailure::Execution(error)),
        AtomicCommitStatus::Pending => {
            return Err(ToolFailure::Execution(
                "atomic file write completion was disconnected".into(),
            ));
        }
    }

    Ok(json!({"tool": "write_file", "path": tool_arguments["path"], "bytes": content_size}))
}

pub(super) async fn create_directory_from_workspace(
    tool_arguments: &Value,
    workspace_directory: filesystem::WorkspaceFile,
) -> Result<Value, ToolFailure> {
    workspace_directory
        .create_directory_all()
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "create_directory", "path": tool_arguments["path"]}))
}

pub(super) async fn rename_path_from_workspace(
    tool_arguments: &Value,
    source_file: filesystem::WorkspaceFile,
    destination_file: filesystem::WorkspaceFile,
) -> Result<Value, ToolFailure> {
    source_file
        .rename_without_replacement(&destination_file)
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({
        "tool": "rename_path",
        "source": tool_arguments["source"],
        "destination": tool_arguments["destination"]
    }))
}

pub(super) async fn delete_path_from_workspace(
    tool_arguments: &Value,
    workspace_entry: filesystem::WorkspaceFile,
) -> Result<Value, ToolFailure> {
    workspace_entry
        .remove_entry()
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "delete_path", "path": tool_arguments["path"]}))
}

fn filesystem_operation_failure(error: filesystem::FilesystemError) -> ToolFailure {
    ToolFailure::Execution(error.to_string())
}
