use app_lib::validate_application_configuration_value;
use serde_json::Value;

#[test]
fn accepts_string_and_number_application_configuration_values() {
    assert!(validate_application_configuration_value(Value::from("en")).is_ok());
    assert!(validate_application_configuration_value(Value::from(1.25)).is_ok());
}

#[test]
fn rejects_other_application_configuration_value_types() {
    assert!(validate_application_configuration_value(Value::Bool(true)).is_err());
    assert!(validate_application_configuration_value(Value::Null).is_err());
}
