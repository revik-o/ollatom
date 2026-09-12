use llm::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub struct OptimizationProvider {
    provider_id: ProviderId,
    history: Vec<ConversationMessage>,
    recorded_requests: Arc<Mutex<Vec<ProviderRunRequest>>>,
    optimization_outcome: Arc<Mutex<Option<ContextOptimizationOutcome>>>,
    summary_call_count: Arc<AtomicUsize>,
}

impl OptimizationProvider {
    pub fn new(history: Vec<ConversationMessage>) -> Arc<Self> {
        Arc::new(Self {
            provider_id: ProviderId::from(BuiltInProvider::Ollama),
            history,
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            optimization_outcome: Arc::new(Mutex::new(None)),
            summary_call_count: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn recorded_requests(&self) -> Vec<ProviderRunRequest> {
        self.recorded_requests.lock().unwrap().clone()
    }

    pub fn optimization_outcome(&self) -> ContextOptimizationOutcome {
        self.optimization_outcome.lock().unwrap().clone().unwrap()
    }

    pub fn summary_call_count(&self) -> usize {
        self.summary_call_count.load(Ordering::SeqCst)
    }
}

impl LlmProvider for OptimizationProvider {
    fn id(&self) -> &ProviderId {
        &self.provider_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    fn availability(
        &self,
        model: Option<&ModelId>,
    ) -> BoxFuture<'_, Result<AvailabilityReport, LlmError>> {
        let provider = self.provider_id.clone();
        let selected_model = model.cloned();
        Box::pin(async move {
            Ok(AvailabilityReport {
                provider,
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
        let model_information = model_information(self.provider_id.clone());
        Box::pin(async move { Ok(vec![model_information]) })
    }

    fn model_info(&self, _model: &ModelId) -> BoxFuture<'_, Result<ModelInfo, LlmError>> {
        let model_information = model_information(self.provider_id.clone());
        Box::pin(async move { Ok(model_information) })
    }

    fn run(
        &self,
        request: ProviderRunRequest,
        host: Arc<dyn ProviderRunHost>,
        _stop: StopToken,
    ) -> BoxFuture<'_, Result<ProviderRunOutcome, LlmError>> {
        self.recorded_requests.lock().unwrap().push(request.clone());
        let is_summary_request = request.limits.provider_rounds == 1
            && request.limits.total_tool_calls == 0
            && request.tools.is_empty();
        let history = self.history.clone();
        let optimization_outcome = self.optimization_outcome.clone();
        let summary_call_count = self.summary_call_count.clone();

        Box::pin(async move {
            host.begin_round(1).await?;

            if is_summary_request {
                summary_call_count.fetch_add(1, Ordering::SeqCst);
                let usage = Usage {
                    source: UsageSource::ApiReported,
                    input_tokens: Some(5),
                    output_tokens: Some(2),
                    total_tokens: Some(7),
                    ..Usage::default()
                };
                host.emit(RunEvent::ResponseDelta("compact summary".into()))
                    .await?;
                host.emit(RunEvent::Usage(usage.clone())).await?;
                return Ok(ProviderRunOutcome::Completed(response(
                    &request,
                    "compact summary",
                    usage,
                )));
            }

            let outcome = host
                .optimize_context(ContextOptimizationRequest {
                    history,
                    observed_input_tokens: 900,
                    input_token_limit: 1_000,
                })
                .await?;
            *optimization_outcome.lock().unwrap() = Some(outcome);
            Ok(ProviderRunOutcome::Completed(response(
                &request,
                "outer response",
                Usage::default(),
            )))
        })
    }
}

pub fn optimization_history() -> Vec<ConversationMessage> {
    vec![
        message(ConversationRole::User, ContentBlock::text("prior goal")),
        message(
            ConversationRole::Assistant,
            ContentBlock::text("prior answer"),
        ),
        message(ConversationRole::User, ContentBlock::text("active goal")),
        tool_call_message("old-call"),
        tool_result_message("old-call"),
        message(
            ConversationRole::Assistant,
            ContentBlock::text("older state"),
        ),
        tool_call_message("recent-call-one"),
        tool_result_message("recent-call-one"),
        tool_call_message("recent-call-two"),
        tool_result_message("recent-call-two"),
        message(
            ConversationRole::Assistant,
            ContentBlock::text("recent state"),
        ),
    ]
}

fn tool_call_message(call_id: &str) -> ConversationMessage {
    message(
        ConversationRole::Assistant,
        ContentBlock::ToolCall {
            call: ToolCall {
                id: call_id.into(),
                name: "echo".into(),
                arguments: serde_json::json!({"call": call_id}),
            },
        },
    )
}

fn tool_result_message(call_id: &str) -> ConversationMessage {
    message(
        ConversationRole::Tool,
        ContentBlock::ToolResult {
            output: ToolOutput::success(call_id, serde_json::json!({"completed": true})),
        },
    )
}

fn message(role: ConversationRole, content: ContentBlock) -> ConversationMessage {
    ConversationMessage {
        role,
        content: vec![content],
    }
}

fn model_information(provider: ProviderId) -> ModelInfo {
    ModelInfo {
        provider,
        id: ModelId::new("test-model").unwrap(),
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

fn response(request: &ProviderRunRequest, text: &str, usage: Usage) -> LlmResponse {
    LlmResponse {
        text: text.into(),
        visible_reasoning: None,
        provider: request.provider.clone(),
        model: request.model.clone(),
        finish_reason: FinishReason::Stop,
        tool_executions: Vec::new(),
        usage,
    }
}
