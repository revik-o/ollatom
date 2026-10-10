use super::LlmRuntime;
use crate::{LlmError, ToolDefinition};
use std::collections::BTreeSet;

pub(super) fn validate_policy(
    definitions: &[ToolDefinition],
    has_approval_path: bool,
) -> Result<(), LlmError> {
    let requires_explicit_user_approval = definitions.iter().any(|definition| {
        definition
            .authorization_requirement
            .requires_explicit_user_approval()
    });

    if requires_explicit_user_approval && !has_approval_path {
        return Err(LlmError::InvalidRequest(
            "tools requiring explicit user approval need a configured approval path".into(),
        ));
    }

    Ok(())
}

pub(super) fn resolve_selected_tool_definitions(
    runtime: &LlmRuntime,
    names: &BTreeSet<String>,
) -> Result<Vec<ToolDefinition>, LlmError> {
    names
        .iter()
        .map(|name| {
            if let Some(definition) = crate::builtins::built_in_tool_definition(name) {
                let is_subagent_tool = name == crate::builtins::INVOKE_SUBAGENT_TOOL_NAME;
                let subagents_are_unconfigured =
                    runtime.inner.profiles.is_empty() || runtime.inner.subagent_runner.is_none();

                if is_subagent_tool && subagents_are_unconfigured {
                    return Err(LlmError::InvalidRequest(
                        "invoke_subagent requires profiles and a runner".into(),
                    ));
                }

                return Ok(definition);
            }

            runtime.inner.tools.definition_for_name(name)
        })
        .collect()
}

pub(super) fn validate_tool_limits(
    names: &BTreeSet<String>,
    limits: crate::RunLimits,
) -> Result<(), LlmError> {
    let tools_are_selected = !names.is_empty();
    let tool_calls_are_disabled = limits.total_tool_calls == 0 || limits.tool_calls_per_round == 0;

    if tools_are_selected && tool_calls_are_disabled {
        return Err(LlmError::InvalidRequest(
            "selected tools require non-zero tool-call limits".into(),
        ));
    }

    let subagent_tool_is_selected = names.contains(crate::builtins::INVOKE_SUBAGENT_TOOL_NAME);
    let subagents_are_disabled = limits.child_subagents == 0 || limits.subagent_depth == 0;

    if subagent_tool_is_selected && subagents_are_disabled {
        return Err(LlmError::InvalidRequest(
            "invoke_subagent requires non-zero child count and depth limits".into(),
        ));
    }

    Ok(())
}
