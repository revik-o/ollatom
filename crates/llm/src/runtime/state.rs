use super::builder::LlmRuntimeBuilder;
use super::interactions::InteractionHub;
use crate::{
    InteractionId, InteractionReply, IntoProviderId, LlmError, LlmProvider, ModelId, ProviderId, RequestBuilder,
    RunEventSink, StopHandle, SubagentProfileRegistry, ToolRegistry, ids::allocate_monotonic_identifier,
    subagent::SharedSubagentRunner,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

pub(crate) struct ProviderRegistration {
    pub provider: Arc<dyn LlmProvider>,
    pub default_model: Option<ModelId>,
}

pub(crate) struct RuntimeInner {
    pub providers: BTreeMap<ProviderId, ProviderRegistration>,
    pub tools: Arc<ToolRegistry>,
    pub event_sink: Option<Arc<dyn RunEventSink>>,
    pub interactions: Arc<InteractionHub>,
    pub profiles: Arc<SubagentProfileRegistry>,
    pub subagent_runner: Option<SharedSubagentRunner>,
    pub authorizer: Option<Arc<dyn crate::ToolAuthorizer>>,
    pub next_run_id: AtomicU64,
    pub active: Mutex<BTreeMap<crate::RunId, StopHandle>>,
}

#[derive(Clone)]
pub struct LlmRuntime {
    pub(crate) inner: Arc<RuntimeInner>,
}

impl LlmRuntime {
    pub fn builder() -> LlmRuntimeBuilder {
        LlmRuntimeBuilder::new()
    }

    pub fn request<P: IntoProviderId>(&self, provider: P) -> RequestBuilder<crate::MissingUserMessage> {
        RequestBuilder::new(Some(self.clone()), provider)
    }

    pub fn cancel(&self, run_id: impl Into<crate::RunId>) -> bool {
        let run_identifier = run_id.into();
        self.inner.active.lock().ok().is_some_and(|active_runs| {
            active_runs.get(&run_identifier).is_some_and(|stop_handle| {
                stop_handle.stop();
                true
            })
        })
    }

    pub fn resolve_interaction(&self, interaction_id: InteractionId, reply: InteractionReply) -> Result<(), LlmError> {
        self.inner.interactions.resolve_interaction(interaction_id, reply)
    }

    pub(crate) fn next_run_id(&self) -> Result<crate::RunId, LlmError> {
        Ok(crate::RunId(allocate_monotonic_identifier(
            &self.inner.next_run_id,
            "run",
        )?))
    }

    pub(crate) fn register_run(&self, run_id: crate::RunId, stop_handle: StopHandle) -> Result<(), LlmError> {
        self.inner
            .active
            .lock()
            .map_err(|_| LlmError::ProviderProtocol("active run lock poisoned".into()))?
            .insert(run_id, stop_handle);

        Ok(())
    }

    pub(crate) fn unregister_run(&self, run_id: crate::RunId) {
        if let Ok(mut active) = self.inner.active.lock() {
            active.remove(&run_id);
        }
    }

    pub(crate) fn provider_registration(&self, provider: &ProviderId) -> Result<&ProviderRegistration, LlmError> {
        self.inner
            .providers
            .get(provider)
            .ok_or_else(|| LlmError::ProviderNotRegistered(provider.clone()))
    }

    pub(crate) fn resolve_selected_or_default_model(
        &self,
        provider: &ProviderId,
        selected_model: Option<&ModelId>,
    ) -> Result<(&ProviderRegistration, ModelId), LlmError> {
        let registration = self.provider_registration(provider)?;
        let model = selected_model
            .cloned()
            .or_else(|| registration.default_model.clone())
            .ok_or_else(|| LlmError::ModelRequired(provider.clone()))?;

        Ok((registration, model))
    }
}
