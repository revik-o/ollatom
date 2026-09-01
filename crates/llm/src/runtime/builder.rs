use super::interactions::InteractionHub;
use super::{LlmRuntime, ProviderRegistration, RuntimeInner};
use crate::{
    IntoModelId, LlmError, LlmProvider, ModelId, RunEventSink, SubagentProfileRegistry,
    SubagentRunner, ToolRegistry,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

#[must_use]
pub struct LlmRuntimeBuilder {
    providers: Vec<(Arc<dyn LlmProvider>, Option<ModelId>)>,
    tools: ToolRegistry,
    event_sink: Option<Arc<dyn RunEventSink>>,
    profiles: SubagentProfileRegistry,
    subagent_runner: Option<Arc<dyn SubagentRunner>>,
    authorizer: Option<Arc<dyn crate::ToolAuthorizer>>,
    validation_error: Option<LlmError>,
}

impl LlmRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            tools: ToolRegistry::new(),
            event_sink: None,
            profiles: SubagentProfileRegistry::default(),
            subagent_runner: None,
            authorizer: None,
            validation_error: None,
        }
    }

    pub fn provider(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        self.providers.push((provider, None));
        self
    }

    pub fn provider_with_default(
        mut self,
        provider: Arc<dyn LlmProvider>,
        model: impl IntoModelId,
    ) -> Self {
        match model.into_model_id() {
            Ok(model) => self.providers.push((provider, Some(model))),
            Err(error) => {
                self.providers.push((provider, None));
                self.set_error(error);
            }
        }

        self
    }

    pub fn tools(mut self, tool_registry: ToolRegistry) -> Self {
        self.tools = tool_registry;
        self
    }

    pub fn event_sink(mut self, event_sink: Arc<dyn RunEventSink>) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    pub fn subagent_profiles(mut self, subagent_profiles: SubagentProfileRegistry) -> Self {
        self.profiles = subagent_profiles;
        self
    }

    pub fn subagent_runner(mut self, subagent_runner: Arc<dyn SubagentRunner>) -> Self {
        self.subagent_runner = Some(subagent_runner);
        self
    }

    pub fn tool_authorizer(mut self, tool_authorizer: Arc<dyn crate::ToolAuthorizer>) -> Self {
        self.authorizer = Some(tool_authorizer);
        self
    }

    pub fn build(self) -> Result<LlmRuntime, LlmError> {
        if let Some(error) = self.validation_error {
            return Err(error);
        }

        let mut providers = BTreeMap::new();

        for (provider, default_model) in self.providers {
            let provider_id = provider.id().clone();

            let provider_was_already_registered = providers
                .insert(
                    provider_id.clone(),
                    ProviderRegistration {
                        provider,
                        default_model,
                    },
                )
                .is_some();
            if provider_was_already_registered {
                return Err(LlmError::DuplicateProvider(provider_id));
            }
        }

        Ok(LlmRuntime {
            inner: Arc::new(RuntimeInner {
                providers,
                tools: Arc::new(self.tools),
                event_sink: self.event_sink,
                interactions: Arc::new(InteractionHub::new()),
                profiles: Arc::new(self.profiles),
                subagent_runner: self.subagent_runner,
                authorizer: self.authorizer,
                next_run_id: AtomicU64::new(1),
                active: Mutex::new(BTreeMap::new()),
            }),
        })
    }

    fn set_error(&mut self, error: LlmError) {
        if self.validation_error.is_none() {
            self.validation_error = Some(error);
        }
    }
}

impl Default for LlmRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
