use super::content::{NativeFunctionCall, NativeToolCall, ToolArgumentAssembly};
use llm::{LlmError, ToolCall};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn merge_native_tool_calls(
    native_tool_calls: &[NativeToolCall],
    accumulated_tool_calls: &mut BTreeMap<usize, NativeToolCall>,
) -> Result<(), LlmError> {
    for (fallback_call_index, native_tool_call) in native_tool_calls.iter().enumerate() {
        let tool_call_index = native_tool_call.function.index.unwrap_or(fallback_call_index);

        let accumulated_tool_call = accumulated_tool_calls
            .entry(tool_call_index)
            .or_insert_with(|| NativeToolCall {
                id: native_tool_call.id.clone(),
                function: NativeFunctionCall {
                    index: Some(tool_call_index),
                    name: native_tool_call.function.name.clone(),
                    arguments: ToolArgumentAssembly::Uninitialized,
                },
            });

        if native_tool_call.id.is_some() {
            accumulated_tool_call.id.clone_from(&native_tool_call.id);
        }

        if !native_tool_call.function.name.is_empty() {
            accumulated_tool_call
                .function
                .name
                .clone_from(&native_tool_call.function.name);
        }

        merge_arguments(
            &mut accumulated_tool_call.function.arguments,
            native_tool_call.function.arguments.clone(),
        )?;
    }

    Ok(())
}

fn merge_arguments(current: &mut ToolArgumentAssembly, next: ToolArgumentAssembly) -> Result<(), LlmError> {
    match (current, next) {
        (current @ ToolArgumentAssembly::Uninitialized, next) => {
            *current = next;
            Ok(())
        }
        (
            ToolArgumentAssembly::StringFragments(current_fragments),
            ToolArgumentAssembly::StringFragments(next_fragments),
        ) => {
            current_fragments.push_str(&next_fragments);
            Ok(())
        }
        (ToolArgumentAssembly::CompleteObject(current_object), ToolArgumentAssembly::CompleteObject(next_object))
            if current_object.is_empty() =>
        {
            *current_object = next_object;
            Ok(())
        }
        (ToolArgumentAssembly::CompleteObject(current_object), ToolArgumentAssembly::CompleteObject(next_object))
            if current_object == &next_object =>
        {
            Ok(())
        }
        _ => Err(LlmError::ProviderProtocol(
            "Ollama tool arguments changed representation during streaming".into(),
        )),
    }
}

pub(super) fn normalize_tool_call(
    native_tool_call: NativeToolCall,
    run_identifier: llm::RunId,
    round_number: u16,
    call_index: usize,
    observed_call_identifiers: &mut BTreeSet<String>,
) -> Result<ToolCall, LlmError> {
    if native_tool_call.function.name.is_empty() {
        return Err(LlmError::ProviderProtocol(
            "Ollama returned a tool call without a function name".into(),
        ));
    }

    let proposed_identifier = native_tool_call.id.filter(|value| !value.is_empty());
    let mut tool_call_identifier = proposed_identifier
        .filter(|value| !observed_call_identifiers.contains(value))
        .unwrap_or_else(|| format!("ollama-{}-{round_number}-{call_index}", run_identifier.0));

    if observed_call_identifiers.contains(&tool_call_identifier) {
        let base_identifier = tool_call_identifier.clone();
        let mut collision_suffix = 1_u32;

        while observed_call_identifiers.contains(&tool_call_identifier) {
            tool_call_identifier = format!("{base_identifier}-{collision_suffix}");
            collision_suffix = collision_suffix
                .checked_add(1)
                .ok_or_else(|| LlmError::ProviderProtocol("Ollama tool-call identifier collision overflow".into()))?;
        }
    }

    observed_call_identifiers.insert(tool_call_identifier.clone());

    let tool_arguments = match native_tool_call.function.arguments {
        ToolArgumentAssembly::StringFragments(arguments) => serde_json::from_str(&arguments)
            .map_err(|_| LlmError::InvalidToolArguments(native_tool_call.function.name.clone()))?,
        ToolArgumentAssembly::CompleteObject(arguments) => Value::Object(arguments),
        ToolArgumentAssembly::Uninitialized => Value::Object(Map::new()),
    };

    if !tool_arguments.is_object() {
        return Err(LlmError::InvalidToolArguments(native_tool_call.function.name));
    }

    Ok(ToolCall {
        id: tool_call_identifier,
        name: native_tool_call.function.name,
        arguments: tool_arguments,
    })
}
