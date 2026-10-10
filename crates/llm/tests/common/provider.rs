use llm::*;
use std::sync::{Arc, Mutex};
mod behavior;
use behavior::execute_behavior;

#[derive(Clone)]
pub enum ProviderBehavior {
    Complete,
    WaitForCancellation,
    StreamUntilCancellation,
    RequestUserInput,
    InvokeSubagent(InvokeSubagentRequest),
    ExecuteToolRound(Vec<ToolCall>),
    RetryToolRoundAfterFinishedFailure(ToolCall),
    StreamAndFailAfterToolRound(ToolCall),
    BeginProviderRounds(u16),
    ReturnMultipleAssistantRounds,
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

    fn availability<'a>(
        &'a self,
        model: Option<&'a ModelId>,
        _transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<AvailabilityReport, LlmError>> {
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

    fn list_models<'a>(
        &'a self,
        _scope: ModelScope,
        _transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<Vec<ModelInfo>, LlmError>> {
        let provider_id = self.provider_id.clone();
        Box::pin(async move { Ok(vec![provider_model_information(provider_id, "test-model")]) })
    }

    fn model_info<'a>(
        &'a self,
        model: &'a ModelId,
        _transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<ModelInfo, LlmError>> {
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

fn completed_response(request: &ProviderRunRequest, text: &str) -> LlmResponse {
    LlmResponse {
        provider: request.provider.clone(),
        model: request.model.clone(),
        assistant_rounds: vec![AssistantRound {
            round_number: 1,
            text: text.into(),
            finish_reason: FinishReason::Stop,
            ..AssistantRound::default()
        }],
        tool_executions: vec![],
        usage: Usage::default(),
    }
}

fn partial_response(request: &ProviderRunRequest, text: &str) -> PartialResponse {
    PartialResponse {
        provider: request.provider.clone(),
        model: request.model.clone(),
        assistant_rounds: vec![AssistantRound {
            round_number: 1,
            text: text.into(),
            finish_reason: FinishReason::Cancelled,
            ..AssistantRound::default()
        }],
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
