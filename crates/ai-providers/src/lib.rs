//! Composition crate for the built-in provider family.

use std::{collections::BTreeMap, sync::Arc};

pub use chatgpt;
pub use claude;
pub use gemini;
pub use llama_cpp;
pub use llm::{
    self, ALL_FILESYSTEM_ACCESS, ALL_USER_COMMANDS, AllowedPermissions, ApprovalDecision,
    ApprovalHandler, ApprovalRequest, AskUserRequest, AuthorizationGrant, AvailabilityReport,
    AvailabilityState, BoxFuture, BuiltInProvider, CapabilitySupport, CommandPattern, ContentBlock,
    ContextOptions, ContextOverflowPolicy, ConversationMessage, ConversationRole, EventCallback,
    EventCallbacks, FilesystemAccess, FinishReason, GenerationOptions, HasUserMessage,
    InteractionId, InteractionReply, InteractionRequest, IntoModelId, IntoProviderId,
    InvokeSubagentRequest, LLM, Llm, LlmError, LlmOptions, LlmProvider, LlmResponse, LlmRun,
    LlmRunOutcome, LlmRuntime, LlmRuntimeBuilder, LocalOptionPhase, LocalRuntimeOptions,
    MissingUserMessage, ModelCapability, ModelId, ModelInfo, ModelScope, ModelState,
    NormalizedValue, OptionHandlingMode, PartialResponse, ProviderCapabilities, ProviderId,
    ProviderRunHost, ProviderRunOutcome, ProviderRunRequest, Question, QuestionAnswer,
    QuestionKind, ReasoningEffort, ReasoningOptions, RequestBuilder, RequiredCapability, RunEvent,
    RunEventSink, RunEventStream, RunId, RunLimits, RunPolicy, SequencedEvent, StopHandle,
    StopToken, SubagentOutcome, SubagentProfile, SubagentProfileId, SubagentProfileRegistry,
    SubagentRunner, Tool, ToolAuthorizer, ToolCall, ToolDefinition, ToolEvent, ToolExecutionRecord,
    ToolFailure, ToolOutput, ToolPlan, ToolRegistry, TransportOptions, TrustedFolder, Usage,
    UsageSource, UserMessage, ValueProvenance,
};
pub use lm_studio;
pub use ollama;

#[must_use]
#[derive(Clone, Debug, Default)]
pub struct BuiltInProviderConfiguration {
    default_models: BTreeMap<ProviderId, ModelId>,
    validation_error: Option<LlmError>,
}

impl BuiltInProviderConfiguration {
    pub fn default_model(mut self, provider: impl IntoProviderId, model: impl IntoModelId) -> Self {
        match (provider.into_provider_id(), model.into_model_id()) {
            (Ok(provider_id), Ok(model_id)) => {
                self.default_models.insert(provider_id, model_id);
            }
            (Err(error), _) | (_, Err(error)) => self.set_error(error),
        }
        self
    }

    fn set_error(&mut self, error: LlmError) {
        if self.validation_error.is_none() {
            self.validation_error = Some(error);
        }
    }
}

pub struct BuiltInProviders;

#[must_use]
pub struct BuiltInProvidersBuilder {
    configuration: BuiltInProviderConfiguration,
    tools: ToolRegistry,
    providers: Vec<Arc<dyn LlmProvider>>,
    subagent_profiles: SubagentProfileRegistry,
    event_sink: Option<Arc<dyn RunEventSink>>,
    subagent_runner: Option<Arc<dyn SubagentRunner>>,
    tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
}

impl BuiltInProviders {
    pub fn builder() -> BuiltInProvidersBuilder {
        BuiltInProvidersBuilder {
            configuration: BuiltInProviderConfiguration::default(),
            tools: ToolRegistry::new(),
            providers: Vec::new(),
            subagent_profiles: SubagentProfileRegistry::default(),
            event_sink: None,
            subagent_runner: None,
            tool_authorizer: None,
        }
    }
}

impl BuiltInProvidersBuilder {
    pub fn configuration(mut self, configuration: BuiltInProviderConfiguration) -> Self {
        self.configuration = configuration;
        self
    }

    pub fn tools(mut self, tool_registry: ToolRegistry) -> Self {
        self.tools = tool_registry;
        self
    }

    pub fn provider(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn subagent_profiles(mut self, subagent_profiles: SubagentProfileRegistry) -> Self {
        self.subagent_profiles = subagent_profiles;
        self
    }

    pub fn event_sink(mut self, event_sink: Arc<dyn RunEventSink>) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    pub fn subagent_runner(mut self, subagent_runner: Arc<dyn SubagentRunner>) -> Self {
        self.subagent_runner = Some(subagent_runner);
        self
    }

    pub fn tool_authorizer(mut self, tool_authorizer: Arc<dyn ToolAuthorizer>) -> Self {
        self.tool_authorizer = Some(tool_authorizer);
        self
    }

    pub fn build(self) -> Result<LlmRuntime, LlmError> {
        if let Some(error) = self.configuration.validation_error {
            return Err(error);
        }

        let mut runtime_builder = LlmRuntime::builder()
            .tools(self.tools)
            .subagent_profiles(self.subagent_profiles);

        if let Some(event_sink) = self.event_sink {
            runtime_builder = runtime_builder.event_sink(event_sink);
        }

        if let Some(subagent_runner) = self.subagent_runner {
            runtime_builder = runtime_builder.subagent_runner(subagent_runner);
        }

        if let Some(tool_authorizer) = self.tool_authorizer {
            runtime_builder = runtime_builder.tool_authorizer(tool_authorizer);
        }

        let mut default_models = self.configuration.default_models;

        for provider in self.providers {
            runtime_builder = match default_models.remove(provider.id()) {
                Some(default_model) => {
                    runtime_builder.provider_with_default(provider, default_model)
                }
                None => runtime_builder.provider(provider),
            };
        }

        if let Some(unregistered_provider_id) = default_models.keys().next() {
            return Err(LlmError::ProviderNotRegistered(
                unregistered_provider_id.clone(),
            ));
        }

        runtime_builder.build()
    }
}
