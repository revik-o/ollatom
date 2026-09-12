mod context_optimization_support;

use context_optimization_support::{OptimizationProvider, optimization_history};
use llm::*;

#[test]
fn context_options_without_the_optimization_field_remain_deserializable() {
    let context_options: ContextOptions = serde_json::from_value(serde_json::json!({
        "input_token_budget": null,
        "overflow_policy": null
    }))
    .unwrap();

    assert!(!context_options.optimization);
}

#[tokio::test]
async fn optimization_compacts_history_without_losing_the_active_goal_or_recent_tool_rounds() {
    let original_history = optimization_history();
    let provider = OptimizationProvider::new(original_history.clone());
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "test-model")
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.context.optimization = true;
    let mut run = runtime
        .request("ollama")
        .options(options)
        .system_prompt("outer system")
        .user_message("start")
        .send();
    let mut events = run.take_event_stream().unwrap();
    let LlmRunOutcome::Completed(response) = run.await.unwrap() else {
        panic!("expected completion");
    };
    let mut context_optimized_event = None;

    while let Some(sequenced_event) = events.next().await {
        if matches!(sequenced_event.event, RunEvent::ContextOptimized { .. }) {
            context_optimized_event = Some(sequenced_event.event);
        }
    }

    let outcome = provider.optimization_outcome();
    assert!(outcome.optimized);
    assert_eq!(outcome.preserved_history, original_history);
    assert_eq!(outcome.model_visible_history.len(), 8);
    assert_eq!(outcome.model_visible_history[1], original_history[2]);
    assert_eq!(&outcome.model_visible_history[2..], &original_history[5..]);
    let ContentBlock::Text { text: summary } = &outcome.model_visible_history[0].content[0] else {
        panic!("expected a text summary");
    };
    assert!(summary.contains("compact summary"));
    assert_eq!(outcome.usage.total_tokens, Some(7));
    assert_eq!(provider.summary_call_count(), 1);
    assert_eq!(response.usage.child_usage, vec![outcome.usage.clone()]);
    assert!(matches!(
        context_optimized_event,
        Some(RunEvent::ContextOptimized {
            original_message_count: 11,
            optimized_message_count: 8,
            observed_input_tokens: 900,
            input_token_limit: 1_000,
        })
    ));

    let recorded_requests = provider.recorded_requests();
    assert_eq!(recorded_requests.len(), 2);
    let optimizer_request = &recorded_requests[1];
    assert!(optimizer_request.tools.is_empty());
    assert!(!optimizer_request.options.context.optimization);
    assert_eq!(optimizer_request.limits.provider_rounds, 1);
    assert_eq!(optimizer_request.limits.total_tool_calls, 0);
    assert_ne!(
        optimizer_request.system_prompt.as_deref(),
        Some("outer system")
    );
}

#[tokio::test]
async fn disabled_optimization_returns_lossless_history_without_a_child_request() {
    let original_history = optimization_history();
    let provider = OptimizationProvider::new(original_history.clone());
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "test-model")
        .build()
        .unwrap();
    let mut run = runtime
        .request("ollama")
        .system_prompt("outer system")
        .user_message("start")
        .send();
    let mut events = run.take_event_stream().unwrap();
    let LlmRunOutcome::Completed(response) = run.await.unwrap() else {
        panic!("expected completion");
    };
    let mut optimization_event_was_emitted = false;

    while let Some(sequenced_event) = events.next().await {
        optimization_event_was_emitted |=
            matches!(sequenced_event.event, RunEvent::ContextOptimized { .. });
    }

    let outcome = provider.optimization_outcome();
    assert!(!outcome.optimized);
    assert_eq!(outcome.model_visible_history, original_history);
    assert_eq!(outcome.model_visible_history, outcome.preserved_history);
    assert_eq!(outcome.usage, Usage::default());
    assert_eq!(provider.summary_call_count(), 0);
    assert_eq!(provider.recorded_requests().len(), 1);
    assert!(response.usage.child_usage.is_empty());
    assert!(!optimization_event_was_emitted);
}
