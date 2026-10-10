use super::*;

pub(super) async fn execute_behavior(
    behavior: ProviderBehavior,
    request: ProviderRunRequest,
    host: Arc<dyn ProviderRunHost>,
    stop: StopToken,
) -> Result<ProviderRunOutcome, LlmError> {
    host.begin_round(1).await?;
    match behavior {
        ProviderBehavior::Complete => {
            host.emit(RunEvent::ResponseDelta("ok".into())).await?;
            Ok(ProviderRunOutcome::Completed(completed_response(
                &request, "ok",
            )))
        }
        ProviderBehavior::WaitForCancellation => {
            stop.cancelled().await;
            Ok(ProviderRunOutcome::Cancelled(partial_response(
                &request, "partial",
            )))
        }
        ProviderBehavior::StreamUntilCancellation => {
            stream_partial_response(&host).await?;
            stop.cancelled().await;

            Err(LlmError::Cancelled)
        }
        ProviderBehavior::RequestUserInput => request_user_input(&request, host).await,
        ProviderBehavior::ExecuteToolRound(tool_calls) => {
            let tool_outputs = host.execute_tool_round(1, tool_calls).await?;
            let response_text = serde_json::to_string(&tool_outputs).unwrap();

            Ok(ProviderRunOutcome::Completed(completed_response(
                &request,
                &response_text,
            )))
        }
        ProviderBehavior::RetryToolRoundAfterFinishedFailure(tool_call) => {
            match host.execute_tool_round(1, vec![tool_call.clone()]).await {
                Ok(tool_outputs) => {
                    let response_text = serde_json::to_string(&tool_outputs).unwrap();
                    Ok(ProviderRunOutcome::Completed(completed_response(
                        &request,
                        &response_text,
                    )))
                }
                Err(LlmError::EventSink(_)) => {
                    host.begin_round(2).await?;
                    let tool_outputs = host.execute_tool_round(2, vec![tool_call]).await?;
                    let response_text = serde_json::to_string(&tool_outputs).unwrap();
                    Ok(ProviderRunOutcome::Completed(completed_response(
                        &request,
                        &response_text,
                    )))
                }
                Err(error) => Err(error),
            }
        }
        ProviderBehavior::StreamAndFailAfterToolRound(tool_call) => {
            host.emit(RunEvent::ResponseDelta("partial answer".into()))
                .await?;
            host.emit(RunEvent::ReasoningSummaryDelta("partial reasoning".into()))
                .await?;
            host.emit(RunEvent::Usage(Usage {
                source: UsageSource::ApiReported,
                total_tokens: Some(7),
                ..Default::default()
            }))
            .await?;
            host.execute_tool_round(1, vec![tool_call]).await?;
            Err(LlmError::Provider("scripted provider failure".into()))
        }
        ProviderBehavior::InvokeSubagent(subagent_request) => {
            let subagent_outcome = host.invoke_subagent(subagent_request).await?;

            Ok(ProviderRunOutcome::Completed(completed_response(
                &request,
                &subagent_outcome.text,
            )))
        }
        ProviderBehavior::BeginProviderRounds(provider_rounds) => {
            for round in 2..=provider_rounds {
                host.begin_round(round).await?;
            }

            Ok(ProviderRunOutcome::Completed(completed_response(
                &request, "rounds",
            )))
        }
        ProviderBehavior::ReturnMultipleAssistantRounds => {
            host.emit(RunEvent::ResponseDelta("first round".into()))
                .await?;
            host.begin_round(2).await?;
            host.emit(RunEvent::ResponseDelta("second round".into()))
                .await?;

            let mut response = completed_response(&request, "");
            response.assistant_rounds = vec![
                AssistantRound {
                    round_number: 1,
                    text: "first round".into(),
                    finish_reason: FinishReason::Stop,
                    ..AssistantRound::default()
                },
                AssistantRound {
                    round_number: 2,
                    text: "second round".into(),
                    finish_reason: FinishReason::Stop,
                    ..AssistantRound::default()
                },
            ];
            Ok(ProviderRunOutcome::Completed(response))
        }
        ProviderBehavior::ReturnWrongIdentity => {
            let mut response = completed_response(&request, "wrong");
            response.provider = ProviderId::new("other-provider").unwrap();

            Ok(ProviderRunOutcome::Completed(response))
        }
    }
}

async fn stream_partial_response(host: &Arc<dyn ProviderRunHost>) -> Result<(), LlmError> {
    host.emit(RunEvent::ResponseDelta("partial".into())).await?;
    host.emit(RunEvent::ReasoningSummaryDelta("summary".into()))
        .await?;
    host.emit(RunEvent::Usage(Usage {
        source: UsageSource::ApiReported,
        total_tokens: Some(3),
        ..Default::default()
    }))
    .await
}

async fn request_user_input(
    request: &ProviderRunRequest,
    host: Arc<dyn ProviderRunHost>,
) -> Result<ProviderRunOutcome, LlmError> {
    let answers = host
        .ask_user(AskUserRequest {
            questions: vec![Question {
                id: "name".into(),
                prompt: "Name?".into(),
                kind: QuestionKind::Text,
                choices: vec![],
                allow_free_form: true,
            }],
        })
        .await?;
    let response_text = serde_json::to_string(&answers).unwrap();

    Ok(ProviderRunOutcome::Completed(completed_response(
        request,
        &response_text,
    )))
}
