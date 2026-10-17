use serde_json::Value;
use std::collections::BTreeSet;

const MAXIMUM_SCHEMA_BYTES: usize = 256 * 1024;
pub(crate) const MAXIMUM_SCHEMA_DEPTH: usize = 64;
const DRAFT_2020_12_SCHEMA_URI: &str = "https://json-schema.org/draft/2020-12/schema";

const SINGLE_SCHEMA_KEYWORDS: &[&str] = &[
    "additionalProperties",
    "contains",
    "contentSchema",
    "else",
    "if",
    "items",
    "not",
    "propertyNames",
    "then",
    "unevaluatedItems",
    "unevaluatedProperties",
];

const SCHEMA_MAP_KEYWORDS: &[&str] = &[
    "$defs",
    "definitions",
    "dependentSchemas",
    "patternProperties",
    "properties",
];

const SCHEMA_ARRAY_KEYWORDS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];

pub(crate) fn validate_schema_contract(schema: &Value) -> Result<(), String> {
    serialized_schema_byte_count(schema)?;

    validate_schema_node(schema, "$", 0)
}

fn serialized_schema_byte_count(schema: &Value) -> Result<usize, String> {
    let mut pending_values = vec![schema];
    let mut byte_count = 0_usize;

    while let Some(value) = pending_values.pop() {
        match value {
            Value::Null => add_schema_bytes(&mut byte_count, 4)?,
            Value::Bool(false) => add_schema_bytes(&mut byte_count, 5)?,
            Value::Bool(true) => add_schema_bytes(&mut byte_count, 4)?,
            Value::Number(number) => add_schema_bytes(&mut byte_count, number.to_string().len())?,
            Value::String(string) => {
                let serialized_string = serde_json::to_string(string)
                    .map_err(|error| format!("schema string could not be serialized: {error}"))?;
                add_schema_bytes(&mut byte_count, serialized_string.len())?;
            }
            Value::Array(values) => {
                add_schema_bytes(&mut byte_count, serialized_container_overhead(values.len())?)?;
                pending_values.extend(values.iter());
            }
            Value::Object(values) => {
                add_schema_bytes(&mut byte_count, serialized_container_overhead(values.len())?)?;

                for (key, child_value) in values {
                    let serialized_key = serde_json::to_string(key)
                        .map_err(|error| format!("schema property name could not be serialized: {error}"))?;
                    add_schema_bytes(
                        &mut byte_count,
                        serialized_key
                            .len()
                            .checked_add(1)
                            .ok_or_else(schema_size_overflow_error)?,
                    )?;
                    pending_values.push(child_value);
                }
            }
        }
    }

    Ok(byte_count)
}

pub(crate) fn serialized_container_overhead(entry_count: usize) -> Result<usize, String> {
    2_usize
        .checked_add(entry_count.saturating_sub(1))
        .ok_or_else(schema_size_overflow_error)
}

fn schema_size_overflow_error() -> String {
    "schema exceeds the 256 KiB size limit".to_string()
}

fn add_schema_bytes(byte_count: &mut usize, additional_bytes: usize) -> Result<(), String> {
    *byte_count = byte_count
        .checked_add(additional_bytes)
        .filter(|updated_byte_count| *updated_byte_count <= MAXIMUM_SCHEMA_BYTES)
        .ok_or_else(schema_size_overflow_error)?;

    Ok(())
}

fn validate_schema_node(schema: &Value, path: &str, depth: usize) -> Result<(), String> {
    if depth > MAXIMUM_SCHEMA_DEPTH {
        return Err("schema exceeds the maximum nesting depth".into());
    }

    let Some(object) = schema.as_object() else {
        return Ok(());
    };

    if object.contains_key("$ref") || object.contains_key("$dynamicRef") {
        return Err(format!(
            "{path} uses external and dynamic schema references, which are not supported"
        ));
    }

    validate_schema_dialect(object, path)?;
    validate_required_properties(object, path)?;

    for keyword in SINGLE_SCHEMA_KEYWORDS {
        if let Some(child_schema) = object.get(*keyword) {
            validate_schema_node(child_schema, &format!("{path}.{keyword}"), next_schema_depth(depth)?)?;
        }
    }

    for keyword in SCHEMA_MAP_KEYWORDS {
        if let Some(child_schemas) = object.get(*keyword).and_then(Value::as_object) {
            for (name, child_schema) in child_schemas {
                validate_schema_node(
                    child_schema,
                    &format!("{path}.{keyword}.{name}"),
                    next_schema_depth(depth)?,
                )?;
            }
        }
    }

    for keyword in SCHEMA_ARRAY_KEYWORDS {
        if let Some(child_schemas) = object.get(*keyword).and_then(Value::as_array) {
            for (index, child_schema) in child_schemas.iter().enumerate() {
                validate_schema_node(
                    child_schema,
                    &format!("{path}.{keyword}[{index}]"),
                    next_schema_depth(depth)?,
                )?;
            }
        }
    }

    Ok(())
}

pub(crate) fn next_schema_depth(depth: usize) -> Result<usize, String> {
    depth
        .checked_add(1)
        .filter(|next_depth| *next_depth <= MAXIMUM_SCHEMA_DEPTH)
        .ok_or_else(|| "schema exceeds the maximum nesting depth".to_string())
}

fn validate_schema_dialect(schema: &serde_json::Map<String, Value>, path: &str) -> Result<(), String> {
    if let Some(schema_uri) = schema.get("$schema")
        && schema_uri.as_str() != Some(DRAFT_2020_12_SCHEMA_URI)
    {
        return Err(format!("{path} declares an unsupported JSON Schema dialect"));
    }

    if schema.contains_key("$vocabulary") {
        return Err(format!("{path} declares an unsupported JSON Schema vocabulary"));
    }

    Ok(())
}

fn validate_required_properties(schema: &serde_json::Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        return Ok(());
    };

    let properties = schema.get("properties").and_then(Value::as_object);

    let mut seen_required = BTreeSet::new();

    for required_property in required.iter().filter_map(Value::as_str) {
        if !seen_required.insert(required_property) {
            return Err(format!(
                "{path}.required contains duplicate property {required_property}"
            ));
        }

        if !properties.is_some_and(|properties| properties.contains_key(required_property)) {
            return Err(format!(
                "{path}.required refers to property {required_property} absent from properties"
            ));
        }
    }

    Ok(())
}
