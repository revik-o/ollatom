use crate::schema::{MAXIMUM_SCHEMA_DEPTH, next_schema_depth, serialized_container_overhead};
use serde_json::json;

#[test]
fn schema_size_and_depth_arithmetic_reject_overflow() {
    assert!(serialized_container_overhead(usize::MAX).is_err());
    assert_eq!(
        next_schema_depth(MAXIMUM_SCHEMA_DEPTH - 1).unwrap(),
        MAXIMUM_SCHEMA_DEPTH
    );
    assert!(next_schema_depth(MAXIMUM_SCHEMA_DEPTH).is_err());
    assert!(next_schema_depth(usize::MAX).is_err());
}

#[test]
fn schema_contract_rejects_excessive_nesting() {
    let mut schema = json!({});

    for _ in 0..=MAXIMUM_SCHEMA_DEPTH {
        schema = json!({"items": schema});
    }

    assert!(crate::schema::validate_schema_contract(&schema).is_err());
}
