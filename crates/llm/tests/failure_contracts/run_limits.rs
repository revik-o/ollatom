use super::*;

#[tokio::test]
async fn provider_round_limits_are_enforced() {
    let provider = ScriptedProvider::new(BuiltInProvider::LlamaCpp, ProviderBehavior::BeginProviderRounds(3));
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let limits = RunLimits {
        provider_rounds: 2,
        ..Default::default()
    };
    assert!(matches!(
        runtime
            .request("llama-cpp")
            .limits(limits)
            .user_message("loop")
            .send()
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::LoopLimit(_),
            ..
        }))
    ));
}
