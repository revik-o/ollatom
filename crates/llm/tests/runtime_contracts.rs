mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;

#[tokio::test]
async fn configured_default_model_and_ordered_events_work() {
    let provider = ScriptedProvider::new(BuiltInProvider::Ollama, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "qwen3:4b")
        .build()
        .unwrap();
    let mut run = runtime.request("ollama").user_message("hello").send();
    let mut events = run.take_event_stream().unwrap();
    let run_outcome = run.await.unwrap();
    let LlmRunOutcome::Completed(response) = run_outcome else {
        panic!("expected completion")
    };
    assert_eq!(response.model.as_str(), "qwen3:4b");
    let mut sequences = Vec::new();
    while let Some(event) = events.next().await {
        sequences.push(event.sequence);
    }
    assert_eq!(sequences, (1..=sequences.len() as u64).collect::<Vec<_>>());
}

#[tokio::test]
async fn cancellation_returns_partial_and_runtime_aliases_openai() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::ChatGpt,
        ProviderBehavior::WaitForCancellation,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "gpt-test")
        .build()
        .unwrap();
    let run = runtime.request("openai").user_message("hello").send();
    let stop_handle = run.stop_handle();
    stop_handle.stop();
    let LlmRunOutcome::Cancelled(partial_response) = run.await.unwrap() else {
        panic!("expected cancellation")
    };
    assert_eq!(partial_response.provider.as_str(), "chatgpt");
}

#[tokio::test]
async fn runtime_cancel_and_dropping_a_started_run_both_reach_terminal_cancellation() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::Ollama,
        ProviderBehavior::WaitForCancellation,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let run = runtime.request("ollama").user_message("wait").send();
    let run_id = run.id();
    assert!(runtime.cancel(run_id));
    assert!(matches!(run.await.unwrap(), LlmRunOutcome::Cancelled(_)));

    let provider = ScriptedProvider::new(
        BuiltInProvider::Claude,
        ProviderBehavior::WaitForCancellation,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let mut run = runtime.request("claude").user_message("wait").send();
    let mut events = run.take_event_stream().unwrap();
    let run_task = tokio::spawn(run);
    assert!(matches!(
        events.next().await.unwrap().event,
        RunEvent::ProviderRoundStarted { .. }
    ));
    run_task.abort();
    let terminal_event = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while let Some(event) = events.next().await {
            if event.event.is_terminal() {
                return event.event;
            }
        }
        panic!("event stream closed without a terminal event")
    })
    .await
    .unwrap();
    assert!(matches!(terminal_event, RunEvent::Cancelled));
}

#[tokio::test]
async fn overall_timeout_stops_the_run_and_is_reported_distinctly() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::LlamaCpp,
        ProviderBehavior::WaitForCancellation,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.transport.overall_timeout_ms = Some(5);
    assert!(matches!(
        runtime
            .request("llama-cpp")
            .options(options)
            .user_message("wait")
            .send()
            .await,
        Err(LlmError::Timeout(_))
    ));
}

#[tokio::test]
async fn ask_user_can_be_resolved_later_by_the_host() {
    let provider =
        ScriptedProvider::new(BuiltInProvider::Claude, ProviderBehavior::RequestUserInput);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "claude-test")
        .build()
        .unwrap();
    let mut run = runtime
        .request("claude")
        .tools(["ask_user"])
        .user_message("hello")
        .send();
    let mut events = run.take_event_stream().unwrap();
    let runtime_for_reply = runtime.clone();
    let interaction_responder = tokio::spawn(async move {
        while let Some(event) = events.next().await {
            if let RunEvent::InteractionRequested(InteractionRequest::AskUser { id, .. }) =
                event.event
            {
                runtime_for_reply
                    .resolve_interaction(
                        id,
                        InteractionReply::UserAnswers(vec![QuestionAnswer {
                            question_id: "name".into(),
                            values: vec![],
                            free_form: Some("Ada".into()),
                        }]),
                    )
                    .unwrap();
                break;
            }
        }
    });
    assert!(matches!(run.await.unwrap(), LlmRunOutcome::Completed(_)));
    interaction_responder.await.unwrap();
}

#[tokio::test]
async fn ask_user_rejects_the_wrong_reply_type() {
    let provider =
        ScriptedProvider::new(BuiltInProvider::Claude, ProviderBehavior::RequestUserInput);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    assert!(matches!(
        runtime
            .request("claude")
            .tools(["ask_user"])
            .on_interaction(|_| async { InteractionReply::Approval(ApprovalDecision::Deny) })
            .user_message("ask")
            .send()
            .await,
        Err(LlmError::ProviderProtocol(_))
    ));
}

#[tokio::test]
async fn ask_user_waiting_is_cancellable() {
    let provider =
        ScriptedProvider::new(BuiltInProvider::Gemini, ProviderBehavior::RequestUserInput);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let mut run = runtime
        .request("gemini")
        .tools(["ask_user"])
        .user_message("ask")
        .send();
    let stop_handle = run.stop_handle();
    let mut events = run.take_event_stream().unwrap();
    let run_task = tokio::spawn(run);
    while let Some(event) = events.next().await {
        if matches!(event.event, RunEvent::InteractionRequested(_)) {
            stop_handle.stop();
            break;
        }
    }
    assert!(matches!(
        run_task.await.unwrap().unwrap(),
        LlmRunOutcome::Cancelled(_)
    ));
}

#[test]
fn ask_user_requires_well_formed_typed_questions_and_answers() {
    assert!(AskUserRequest { questions: vec![] }.validate().is_err());
    let request = AskUserRequest {
        questions: vec![Question {
            id: "language".into(),
            prompt: "Language?".into(),
            kind: QuestionKind::SingleChoice,
            choices: vec!["Rust".into()],
            allow_free_form: false,
        }],
    };
    request.validate().unwrap();
    let answers_are_invalid = request
        .validate_answers(&[QuestionAnswer {
            question_id: "language".into(),
            values: vec!["Python".into()],
            free_form: None,
        }])
        .is_err();
    assert!(answers_are_invalid);
}
