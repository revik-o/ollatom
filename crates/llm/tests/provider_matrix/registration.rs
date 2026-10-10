use super::*;

#[tokio::test]
async fn an_explicit_model_does_not_require_a_configured_default() {
    let runtime = local_runtime_without_default();
    let outcome = runtime
        .request("lm-studio")
        .model("installed-model")
        .user_message("hello")
        .send()
        .expect("run should start")
        .await
        .unwrap();
    assert!(matches!(outcome, LlmRunOutcome::Completed(_)));
}

#[test]
fn duplicate_provider_registrations_are_rejected() {
    let provider = ScriptedProvider::new(BuiltInProvider::LmStudio, ProviderBehavior::Complete);
    assert!(matches!(
        LlmRuntime::builder()
            .provider(provider.clone())
            .provider(provider)
            .build(),
        Err(LlmError::DuplicateProvider(_))
    ));
}

#[tokio::test]
async fn requests_for_unregistered_providers_are_rejected() {
    let runtime = local_runtime_without_default();
    assert!(matches!(
        runtime.request("unknown").list_models().await,
        Err(LlmError::ProviderNotRegistered(_))
    ));
}
