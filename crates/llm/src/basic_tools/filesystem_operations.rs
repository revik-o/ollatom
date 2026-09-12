use super::{BasicToolConfiguration, tool_support};
use crate::ToolFailure;
use filesystem::FilePointer;
use serde_json::{Value, json};
use std::io;

pub(super) async fn read_file(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    let file_path = tool_support::resolve_path_argument(configuration, tool_arguments, "path")?;
    let file_metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(input_output_failure)?;

    if !file_metadata.is_file() || file_metadata.len() > configuration.maximum_file_size {
        return Err(ToolFailure::InvalidArguments(
            "path is not a supported regular file".into(),
        ));
    }

    let file_content = FilePointer::from_path(file_path)
        .read_text()
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "read_file", "path": tool_arguments["path"], "content": file_content}))
}

pub(super) async fn write_file(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    let file_path = tool_support::resolve_path_argument(configuration, tool_arguments, "path")?;
    let file_content = tool_arguments
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolFailure::InvalidArguments("content must be a string".into()))?;
    let content_size = u64::try_from(file_content.len()).unwrap_or(u64::MAX);

    if content_size > configuration.maximum_file_size {
        return Err(ToolFailure::InvalidArguments("content is too large".into()));
    }

    let parent_directory = file_path
        .parent()
        .ok_or_else(|| ToolFailure::InvalidArguments("path has no parent".into()))?;

    tokio::fs::create_dir_all(parent_directory)
        .await
        .map_err(input_output_failure)?;

    let file_pointer = FilePointer::from_path(file_path);

    file_pointer
        .write_text_atomically(file_content)
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "write_file", "path": tool_arguments["path"], "bytes": content_size}))
}

pub(super) async fn create_directory(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    let directory_path =
        tool_support::resolve_path_argument(configuration, tool_arguments, "path")?;
    let directory_name = directory_path
        .file_name()
        .ok_or_else(|| ToolFailure::InvalidArguments("path has no directory name".into()))?;
    let parent_directory = directory_path
        .parent()
        .ok_or_else(|| ToolFailure::InvalidArguments("path has no parent directory".into()))?;

    filesystem::create_folder(directory_name.to_string_lossy(), parent_directory)
        .await
        .map_err(filesystem_operation_failure)?;

    Ok(json!({"tool": "create_directory", "path": tool_arguments["path"]}))
}

pub(super) async fn rename_path(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    let source_path = tool_support::resolve_path_argument(configuration, tool_arguments, "source")?;
    let destination_path =
        tool_support::resolve_path_argument(configuration, tool_arguments, "destination")?;

    tokio::fs::rename(source_path, destination_path)
        .await
        .map_err(input_output_failure)?;

    Ok(json!({
        "tool": "rename_path",
        "source": tool_arguments["source"],
        "destination": tool_arguments["destination"]
    }))
}

pub(super) async fn delete_path(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
) -> Result<Value, ToolFailure> {
    let entry_path = tool_support::resolve_path_argument(configuration, tool_arguments, "path")?;
    let entry_metadata = tokio::fs::symlink_metadata(&entry_path)
        .await
        .map_err(input_output_failure)?;

    if entry_metadata.is_dir() {
        tokio::fs::remove_dir_all(entry_path)
            .await
            .map_err(input_output_failure)?;
    } else {
        tokio::fs::remove_file(entry_path)
            .await
            .map_err(input_output_failure)?;
    }

    Ok(json!({"tool": "delete_path", "path": tool_arguments["path"]}))
}

fn input_output_failure(error: io::Error) -> ToolFailure {
    ToolFailure::Execution(error.to_string())
}

fn filesystem_operation_failure(error: filesystem::FilesystemError) -> ToolFailure {
    ToolFailure::Execution(error.to_string())
}
