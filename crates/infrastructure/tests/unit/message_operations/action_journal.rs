use crate::message_operations::action_journal::next_action_status_sequence_number;

#[test]
fn next_action_status_sequence_checks_range_and_overflow() {
    let last_valid_sequence = i64::from(u32::MAX) - 1;
    assert_eq!(
        next_action_status_sequence_number(last_valid_sequence).unwrap(),
        u32::MAX
    );
    assert!(next_action_status_sequence_number(i64::from(u32::MAX)).is_err());
    assert!(next_action_status_sequence_number(i64::MAX).is_err());
    assert!(next_action_status_sequence_number(-2).is_err());
}
