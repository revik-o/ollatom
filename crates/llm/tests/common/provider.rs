use llm::*;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum ProviderBehavior {
    Complete,
    WaitForCancellation,
    StreamUntilCancellation,
    RequestUserInput,
    InvokeSubagent(InvokeSubagentRequest),
    ExecuteToolRound(Vec<ToolCall>),
    BeginProviderRounds(u16),
    ReturnWrongIdentity,
}

pub struct ScriptedProvider {
    provider_id: ProviderId,
    behavior: ProviderBehavior,
    capabilities: ProviderCapabilities,
    pub recorded_requests: Mutex<Vec<ProviderRunRequest>>,
}

impl ScriptedProvider {
    pub fn new(provider_id: impl Into<ProviderId>, behavior: ProviderBehavior) -> Arc<Self> {
        Self::with_capabilities(provider_id, behavior, ProviderCapabilities::default())
    }

    pub fn with_capabilities(
        provider_id: impl Into<ProviderId>,
        behavior: ProviderBehavior,
        capabilities: ProviderCapabilities,
    ) -> Arc<Self> {
        Arc::new(Self {
            provider_id: provider_id.into(),
            behavior,
            capabilities,
            recorded_requests: Mutex::new(Vec::new()),
        })
    }
}

impl LlmProvider for ScriptedProvider {
    fn id(&self) -> &ProviderId {
        &self.provider_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }

    fn availability(
        &self,
        model: Option<&ModelId>,
    ) -> BoxFuture<'_, Result<AvailabilityReport, LlmError>> {
        let provider_id = self.provider_id.clone();
        let selected_model = model.cloned();

        Box::pin(async move {
            Ok(AvailabilityReport {
                provider: provider_id,
                state: AvailabilityState::Ready,
                endpoint: AvailabilityState::Ready,
                authentication: AvailabilityState::Ready,
                model: AvailabilityState::Ready,
                selected_model,
                message: None,
            })
        })
    }

    fn list_models(&self, _scope: ModelScope) -> BoxFuture<'_, Result<Vec<ModelInfo>, LlmError>> {
        let provider_id = self.provider_id.clone();
        Box::pin(async move { Ok(vec![provider_model_information(provider_id, "test-model")]) })
    }

    fn model_info(&self, model: &ModelId) -> BoxFuture<'_, Result<ModelInfo, LlmError>> {
        let model_information =
            provider_model_information(self.provider_id.clone(), model.as_str());

        Box::pin(async move { Ok(model_information) })
    }

    fn run(
        &self,
        request: ProviderRunRequest,
        host: Arc<dyn ProviderRunHost>,
        stop: StopToken,
    ) -> BoxFuture<'_, Result<ProviderRunOutcome, LlmError>> {
        let behavior = self.behavior.clone();
        self.recorded_requests.lock().unwrap().push(request.clone());

        Box::pin(async move { execute_behavior(behavior, request, host, stop).await })
    }
}

async fn execute_behavior(
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

fn completed_response(request: &ProviderRunRequest, text: &str) -> LlmResponse {
    LlmResponse {
        text: text.into(),
        visible_reasoning: None,
        provider: request.provider.clone(),
        model: request.model.clone(),
        finish_reason: FinishReason::Stop,
        tool_executions: vec![],
        usage: Usage::default(),
    }
}

fn partial_response(request: &ProviderRunRequest, text: &str) -> PartialResponse {
    PartialResponse {
        text: text.into(),
        visible_reasoning: None,
        provider: request.provider.clone(),
        model: request.model.clone(),
        tool_executions: vec![],
        usage: Usage::default(),
    }
}

fn provider_model_information(provider: ProviderId, model: &str) -> ModelInfo {
    ModelInfo {
        provider,
        id: ModelId::new(model).unwrap(),
        display_name: NormalizedValue::default(),
        context_window: NormalizedValue::default(),
        max_output_tokens: NormalizedValue::default(),
        state: NormalizedValue::default(),
        quantization: NormalizedValue::default(),
        owner: NormalizedValue::default(),
        capabilities: ProviderCapabilities::default(),
        provider_metadata: Default::default(),
    }
}
