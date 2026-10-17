use super::*;

#[tokio::test]
async fn all_six_builtin_ids_share_the_same_facade_contract() {
    let mut builder = LlmRuntime::builder();

    for provider in BuiltInProvider::ALL {
        builder = builder.provider_with_default(
            ScriptedProvider::new(provider, ProviderBehavior::Complete),
            format!("{}-test", provider.as_str()),
        );
    }

    let runtime = builder.build().unwrap();

    for provider in BuiltInProvider::ALL {
        assert!(runtime.request(provider).is_available().await.unwrap());
        assert_eq!(runtime.request(provider).list_models().await.unwrap().len(), 1);
        let model_information = runtime.request(provider).get_info().await.unwrap();
        assert_eq!(model_information.provider, ProviderId::from(provider));
        assert_eq!(model_information.id.as_str(), format!("{}-test", provider.as_str()));
        assert!(matches!(
            runtime
                .request(provider)
                .user_message("hello")
                .send()
                .expect("run should start")
                .await
                .unwrap(),
            LlmRunOutcome::Completed(_)
        ));
    }
}

#[tokio::test]
async fn all_six_builtin_ids_share_the_cancellation_contract() {
    for provider in BuiltInProvider::ALL {
        let runtime = LlmRuntime::builder()
            .provider_with_default(
                ScriptedProvider::new(provider, ProviderBehavior::WaitForCancellation),
                "model",
            )
            .build()
            .unwrap();
        let run = runtime
            .request(provider)
            .user_message("stop")
            .send()
            .expect("run should start");
        run.stop_handle().stop();
        assert!(matches!(run.await.unwrap(), LlmRunOutcome::Cancelled(_)));
    }
}

#[tokio::test]
async fn strict_and_best_effort_option_negotiation_are_distinct() {
    let mut capabilities = ProviderCapabilities::default();
    capabilities
        .generation_options
        .insert("temperature".into(), CapabilitySupport::Unsupported);
    let provider =
        ScriptedProvider::with_capabilities(BuiltInProvider::Ollama, ProviderBehavior::Complete, capabilities);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    let mut strict_options = LlmOptions::default();
    strict_options.generation.temperature = Some(0.5);
    assert!(matches!(
        runtime
            .request("ollama")
            .options(strict_options)
            .user_message("hello")
            .send()
            .expect("run should start")
            .await,
        Err(LlmError::UnsupportedOption(_))
    ));
    let mut best_effort = LlmOptions {
        handling: OptionHandlingMode::BestEffort,
        ..Default::default()
    };
    best_effort.generation.temperature = Some(0.5);
    runtime
        .request("ollama")
        .effort(ReasoningEffort::High)
        .options(best_effort)
        .user_message("hello")
        .send()
        .expect("run should start")
        .await
        .unwrap();
    let recorded_requests = provider.recorded_requests.lock().unwrap();
    let request = recorded_requests.last().unwrap();
    assert_eq!(request.options.generation.temperature, None);
    assert_eq!(request.options.reasoning.effort, ReasoningEffort::High);
}

#[tokio::test]
async fn keep_alive_uses_its_public_capability_name() {
    let mut capabilities = ProviderCapabilities::default();
    capabilities
        .local_runtime_options
        .insert("keep_alive_seconds".into(), CapabilitySupport::Unsupported);
    let provider =
        ScriptedProvider::with_capabilities(BuiltInProvider::Ollama, ProviderBehavior::Complete, capabilities);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let options = LlmOptions {
        local: LocalRuntimeOptions {
            keep_alive_seconds: Some(60),
            ..LocalRuntimeOptions::default()
        },
        ..LlmOptions::default()
    };

    assert!(matches!(
        runtime
            .request(BuiltInProvider::Ollama)
            .options(options)
            .user_message("hello")
            .send()
            .unwrap()
            .await,
        Err(LlmError::UnsupportedOption(option_name))
            if option_name == "keep_alive_seconds"
    ));
}

#[tokio::test]
async fn auto_preserves_native_reasoning_but_explicit_unsupported_effort_fails() {
    let mut capabilities = ProviderCapabilities::default();
    capabilities
        .features
        .insert(ModelCapability::Reasoning, CapabilitySupport::Unsupported);
    let provider =
        ScriptedProvider::with_capabilities(BuiltInProvider::LmStudio, ProviderBehavior::Complete, capabilities);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    runtime
        .request("lm-studio")
        .user_message("native default")
        .send()
        .expect("run should start")
        .await
        .unwrap();
    assert_eq!(
        provider.recorded_requests.lock().unwrap()[0].options.reasoning.effort,
        ReasoningEffort::Auto
    );
    assert!(matches!(
        runtime
            .request("lm-studio")
            .effort(ReasoningEffort::High)
            .user_message("think")
            .send()
            .expect("run should start")
            .await,
        Err(LlmError::UnsupportedEffort(_))
    ));
}

#[test]
fn opaque_provider_state_and_identifiers_round_trip_through_serde() {
    let block = ContentBlock::ProviderOpaque {
        provider: ProviderId::new("claude").unwrap(),
        kind: "thinking_signature".into(),
        data: vec![1, 2, 3],
    };
    let encoded = serde_json::to_string(&block).unwrap();
    assert_eq!(serde_json::from_str::<ContentBlock>(&encoded).unwrap(), block);
    assert!(serde_json::from_str::<ModelId>("\"\"").is_err());
    let usage: Usage = serde_json::from_str("{}").unwrap();
    assert_eq!(usage.source, UsageSource::Unknown);
}

#[tokio::test]
async fn omitted_models_require_a_configured_default() {
    let runtime = local_runtime_without_default();
    assert!(!runtime.request("lm-studio").is_available().await.unwrap());
    assert!(matches!(
        runtime
            .request("lm-studio")
            .user_message("hello")
            .send()
            .expect("run should start")
            .await,
        Err(LlmError::ModelRequired(_))
    ));
}
