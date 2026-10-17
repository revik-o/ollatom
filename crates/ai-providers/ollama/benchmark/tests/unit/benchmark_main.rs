use crate::require_completed_outcome;
use llm::{AssistantRound, BuiltInProvider, FinishReason, LlmResponse, LlmRunOutcome, Usage};

fn completed_run(finish_reasons: &[FinishReason]) -> Result<LlmRunOutcome, llm::LlmError> {
    Ok(LlmRunOutcome::Completed(LlmResponse {
        provider: BuiltInProvider::Ollama.into(),
        model: llm::ModelId::new("test-model")?,
        assistant_rounds: finish_reasons
            .iter()
            .map(|finish_reason| AssistantRound {
                finish_reason: *finish_reason,
                ..AssistantRound::default()
            })
            .collect(),
        tool_executions: Vec::new(),
        usage: Usage::default(),
    }))
}

#[test]
fn benchmark_requires_a_final_normal_stop() {
    assert!(require_completed_outcome(completed_run(&[FinishReason::Stop]), 3).is_ok());
    assert!(require_completed_outcome(completed_run(&[FinishReason::ToolCalls, FinishReason::Stop]), 3,).is_ok());

    for finish_reason in [
        FinishReason::ToolCalls,
        FinishReason::Length,
        FinishReason::ContentFilter,
        FinishReason::Error,
        FinishReason::Cancelled,
        FinishReason::Unknown,
    ] {
        assert!(
            require_completed_outcome(completed_run(&[finish_reason]), 3).is_err(),
            "{finish_reason:?} must not count as benchmark success"
        );
    }

    assert!(require_completed_outcome(completed_run(&[]), 3).is_err());
}
