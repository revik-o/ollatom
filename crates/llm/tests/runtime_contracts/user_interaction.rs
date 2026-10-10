use super::*;

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
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::ProviderProtocol(_),
            ..
        }))
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
        .send()
        .expect("run should start");
    let stop_handle = run.stop_handle();
    let mut events = run.take_event_stream().unwrap();
    let run_task = tokio::spawn(run);
    while let Some(event) = events.next().await {
        if matches!(event.event(), Some(RunEvent::InteractionRequested(_))) {
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
