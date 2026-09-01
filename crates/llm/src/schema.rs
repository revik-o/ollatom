use crate::ToolFailure;
use serde_json::Value;

pub(crate) fn validate_definition(schema: &Value, path: &str) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;

    if let Some(kind) = object.get("type") {
        let kind = kind
            .as_str()
            .ok_or_else(|| format!("{path}.type must be a string"))?;

        if !is_supported_type(kind) {
            return Err(format!("{path}.type contains unsupported type {kind}"));
        }
    }

    if let Some(required) = object.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| format!("{path}.required must be an array"))?;

        if required.iter().any(|value| !value.is_string()) {
            return Err(format!("{path}.required entries must be strings"));
        }
    }

    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| format!("{path}.properties must be an object"))?;

        for (name, child) in properties {
            validate_definition(child, &format!("{path}.properties.{name}"))?;
        }
    }

    if let Some(items) = object.get("items") {
        validate_definition(items, &format!("{path}.items"))?;
    }

    if object.get("enum").is_some_and(|value| !value.is_array()) {
        return Err(format!("{path}.enum must be an array"));
    }

    Ok(())
}

pub(crate) fn validate(schema: &Value, value: &Value, path: &str) -> Result<(), ToolFailure> {
    let enum_rejects_value = schema
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|choices| !choices.contains(value));

    if enum_rejects_value {
        return invalid_arguments(path, "value is not in the allowed enum");
    }

    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        let valid = match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => false,
        };

        if !valid {
            return invalid_arguments(path, &format!("expected {kind}"));
        }
    }

    if let Some(object) = value.as_object() {
        validate_object(schema, object, path)?;
    }

    if let Some(array) = value.as_array() {
        validate_array(schema, array, path)?;
    }

    if let Some(string) = value.as_str() {
        validate_string(schema, string, path)?;
    }

    if let Some(number) = value.as_f64() {
        validate_number(schema, number, path)?;
    }

    Ok(())
}

fn validate_object(
    schema: &Value,
    object: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), ToolFailure> {
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for property in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(property) {
                return invalid_arguments(path, &format!("missing required property {property}"));
            }
        }
    }

    let properties = schema.get("properties").and_then(Value::as_object);
    let rejects_additional_properties =
        schema.get("additionalProperties").and_then(Value::as_bool) == Some(false);

    if rejects_additional_properties {
        let unexpected_property = object
            .keys()
            .find(|name| properties.is_none_or(|known| !known.contains_key(*name)));

        if let Some(name) = unexpected_property {
            return invalid_arguments(path, &format!("unexpected property {name}"));
        }
    }

    if let Some(properties) = properties {
        for (name, child_schema) in properties {
            if let Some(child) = object.get(name) {
                validate(child_schema, child, &format!("{path}.{name}"))?;
            }
        }
    }

    Ok(())
}

fn validate_array(schema: &Value, array: &[Value], path: &str) -> Result<(), ToolFailure> {
    let is_shorter_than_minimum = schema
        .get("minItems")
        .and_then(Value::as_u64)
        .is_some_and(|minimum| array.len() < minimum as usize);

    if is_shorter_than_minimum {
        return invalid_arguments(path, "array is shorter than minItems");
    }

    let is_longer_than_maximum = schema
        .get("maxItems")
        .and_then(Value::as_u64)
        .is_some_and(|maximum| array.len() > maximum as usize);

    if is_longer_than_maximum {
        return invalid_arguments(path, "array is longer than maxItems");
    }

    if let Some(items) = schema.get("items") {
        for (index, item) in array.iter().enumerate() {
            validate(items, item, &format!("{path}[{index}]"))?;
        }
    }

    Ok(())
}

fn validate_string(schema: &Value, string: &str, path: &str) -> Result<(), ToolFailure> {
    let is_shorter_than_minimum = schema
        .get("minLength")
        .and_then(Value::as_u64)
        .is_some_and(|minimum| string.chars().count() < minimum as usize);

    if is_shorter_than_minimum {
        return invalid_arguments(path, "string is shorter than minLength");
    }

    let is_longer_than_maximum = schema
        .get("maxLength")
        .and_then(Value::as_u64)
        .is_some_and(|maximum| string.chars().count() > maximum as usize);

    if is_longer_than_maximum {
        return invalid_arguments(path, "string is longer than maxLength");
    }

    Ok(())
}

fn validate_number(schema: &Value, number: f64, path: &str) -> Result<(), ToolFailure> {
    let is_below_minimum = schema
        .get("minimum")
        .and_then(Value::as_f64)
        .is_some_and(|minimum| number < minimum);

    if is_below_minimum {
        return invalid_arguments(path, "number is below minimum");
    }

    let is_above_maximum = schema
        .get("maximum")
        .and_then(Value::as_f64)
        .is_some_and(|maximum| number > maximum);

    if is_above_maximum {
        return invalid_arguments(path, "number is above maximum");
    }

    Ok(())
}

fn invalid_arguments<T>(path: &str, message: &str) -> Result<T, ToolFailure> {
    Err(ToolFailure::InvalidArguments(format!("{path}: {message}")))
}

fn is_supported_type(kind: &str) -> bool {
    matches!(
        kind,
        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
    )
}
