use crate::LlmError;
use crate::ids::*;
use std::sync::atomic::AtomicU64;

#[test]
fn monotonic_identifier_allocation_fails_before_wrapping() {
    let next_identifier = AtomicU64::new(u64::MAX);

    assert_eq!(
        allocate_monotonic_identifier(&next_identifier, "run").unwrap(),
        u64::MAX
    );
    assert_eq!(
        allocate_monotonic_identifier(&next_identifier, "run"),
        Err(LlmError::IdentifierExhausted("run".to_owned()))
    );
}
