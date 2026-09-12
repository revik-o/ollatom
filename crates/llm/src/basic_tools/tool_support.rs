use super::BasicToolConfiguration;
use crate::{ToolDefinition, ToolFailure, ToolPlan};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) fn create_tool_definition(
    name: &str,
    description: &str,
    properties: Value,
    required_property_names: &[&str],
) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: description.into(),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required_property_names,
            "additionalProperties": false
        }),
        requires_authorization: true,
    }
}

pub(super) fn validate_tool_call(
    tool_definition: &ToolDefinition,
    tool_call: &crate::ToolCall,
) -> Result<(), ToolFailure> {
    crate::tools::validate_arguments(&tool_definition.input_schema, &tool_call.arguments)
}

pub(super) fn resolve_path_argument(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
    argument_name: &str,
) -> Result<PathBuf, ToolFailure> {
    let relative_path = tool_arguments
        .get(argument_name)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolFailure::InvalidArguments(format!("{argument_name} must be a string"))
        })?;

    resolve_path(configuration.root_directory(), relative_path)
}

pub(super) fn resolve_path(
    configured_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ToolFailure> {
    let relative_path_reference = Path::new(relative_path);

    if relative_path.is_empty() || relative_path_reference.is_absolute() {
        return Err(ToolFailure::InvalidArguments(
            "path must be a non-empty relative path".into(),
        ));
    }

    if relative_path_reference
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ToolFailure::InvalidArguments(
            "path must not traverse a parent directory".into(),
        ));
    }

    reject_symlink_components(configured_root, relative_path_reference)?;

    let canonical_root_path = fs::canonicalize(configured_root)
        .map_err(|error| ToolFailure::Execution(error.to_string()))?;
    let candidate_path = configured_root.join(relative_path_reference);
    let mut existing_ancestor_path = candidate_path.clone();
    let mut missing_path_components = Vec::new();
    let canonical_candidate_path = loop {
        match fs::canonicalize(&existing_ancestor_path) {
            Ok(canonical_existing_ancestor_path) => {
                let mut canonical_path = canonical_existing_ancestor_path;

                for component in missing_path_components.iter().rev() {
                    canonical_path.push(component);
                }

                break canonical_path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let missing_path_component = existing_ancestor_path
                    .file_name()
                    .ok_or_else(|| ToolFailure::InvalidArguments("path has no file name".into()))?
                    .to_owned();

                missing_path_components.push(missing_path_component);

                if !existing_ancestor_path.pop() {
                    return Err(ToolFailure::Execution(
                        "path parent could not be resolved".into(),
                    ));
                }
            }
            Err(error) => return Err(ToolFailure::Execution(error.to_string())),
        }
    };

    if !canonical_candidate_path.starts_with(&canonical_root_path) {
        return Err(ToolFailure::InvalidArguments(
            "path escapes the configured root".into(),
        ));
    }

    if canonical_candidate_path == canonical_root_path {
        return Err(ToolFailure::InvalidArguments(
            "path must identify an entry beneath the configured root".into(),
        ));
    }

    Ok(candidate_path)
}

fn reject_symlink_components(
    configured_root: &Path,
    relative_path: &Path,
) -> Result<(), ToolFailure> {
    let mut inspected_path = configured_root.to_owned();

    for component in relative_path.components() {
        let Component::Normal(component) = component else {
            continue;
        };

        inspected_path.push(component);

        match fs::symlink_metadata(&inspected_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ToolFailure::InvalidArguments(
                    "path must not contain symlink components".into(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(ToolFailure::Execution(error.to_string())),
        }
    }

    Ok(())
}

pub(super) fn filesystem_capability(
    filesystem_path: PathBuf,
    filesystem_access: crate::FilesystemAccess,
) -> crate::RequiredCapability {
    crate::RequiredCapability::Filesystem {
        path: filesystem_path,
        access: filesystem_access,
    }
}

pub(super) fn create_tool_plan(
    tool_call: &crate::ToolCall,
    normalized_arguments: Value,
    required_capabilities: Vec<crate::RequiredCapability>,
    timeout: Option<std::time::Duration>,
) -> ToolPlan {
    ToolPlan {
        call: tool_call.clone(),
        normalized_arguments,
        required_capabilities,
        timeout,
    }
}
