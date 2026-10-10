use llm::{Usage, UsageOverflow};

#[test]
fn token_usage_overflow_is_reported() {
    let usage = Usage {
        input_tokens: Some(u64::MAX),
        output_tokens: Some(1),
        ..Usage::default()
    };

    assert_eq!(usage.total_tokens_including_children(), Err(UsageOverflow));
}

#[test]
fn child_token_usage_overflow_is_reported() {
    let usage = Usage {
        total_tokens: Some(u64::MAX),
        child_usage: vec![Usage {
            total_tokens: Some(1),
            ..Usage::default()
        }],
        ..Usage::default()
    };

    assert_eq!(usage.total_tokens_including_children(), Err(UsageOverflow));
}
