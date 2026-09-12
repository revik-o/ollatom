use super::content::{NativeFunctionCall, NativeToolCall};
use llm::{LlmError, ToolCall};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn merge_native_tool_calls(
    native_tool_call_values: &[Value],
    accumulated_tool_calls: &mut BTreeMap<usize, NativeToolCall>,
) -> Result<(), LlmError> {
    for (fallback_call_index, native_tool_call_value) in native_tool_call_values.iter().enumerate()
    {
        let native_tool_call: NativeToolCall =
            serde_json::from_value(native_tool_call_value.clone())
                .map_err(|_| LlmError::ProviderProtocol("invalid Ollama tool call".into()))?;
        let tool_call_index = native_tool_call
            .function
            .index
            .unwrap_or(fallback_call_index);
        let accumulated_tool_call = accumulated_tool_calls
            .entry(tool_call_index)
            .or_insert_with(|| NativeToolCall {
                id: native_tool_call.id.clone(),
                function: NativeFunctionCall {
                    index: Some(tool_call_index),
                    name: native_tool_call.function.name.clone(),
                    arguments: Value::Object(Map::new()),
                },
            });

        if native_tool_call.id.is_some() {
            accumulated_tool_call.id = native_tool_call.id;
        }

        if !native_tool_call.function.name.is_empty() {
            accumulated_tool_call.function.name = native_tool_call.function.name;
        }

        accumulated_tool_call.function.arguments = merge_arguments(
            &accumulated_tool_call.function.arguments,
            &native_tool_call.function.arguments,
        )?;
    }

    Ok(())
}

fn merge_arguments(current: &Value, next: &Value) -> Result<Value, LlmError> {
    if next.is_object() {
        return Ok(next.clone());
    }

    if let Some(next_string) = next.as_str() {
        let current_string = current.as_str().unwrap_or_default();
        let combined = format!("{current_string}{next_string}");
        return serde_json::from_str(&combined).or(Ok(Value::String(combined)));
    }

    Ok(next.clone())
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
            collision_suffix += 1;
        }
    }

    observed_call_identifiers.insert(tool_call_identifier.clone());

    let tool_arguments = if native_tool_call.function.arguments.is_string() {
        serde_json::from_str(
            native_tool_call
                .function
                .arguments
                .as_str()
                .unwrap_or_default(),
        )
        .map_err(|_| LlmError::InvalidToolArguments(native_tool_call.function.name.clone()))?
    } else {
        native_tool_call.function.arguments
    };

    if !tool_arguments.is_object() {
        return Err(LlmError::InvalidToolArguments(
            native_tool_call.function.name,
        ));
    }

    Ok(ToolCall {
        id: tool_call_identifier,
        name: native_tool_call.function.name,
        arguments: tool_arguments,
    })
}
