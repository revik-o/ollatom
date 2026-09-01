use super::builder::LlmRuntimeBuilder;
use super::interactions::InteractionHub;
use crate::{
    InteractionId, InteractionReply, IntoProviderId, LlmError, LlmProvider, ModelId, ProviderId,
    RequestBuilder, RunEventSink, StopHandle, SubagentProfileRegistry, ToolRegistry,
    subagent::SharedSubagentRunner,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
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

    pub fn request<P: IntoProviderId>(
        &self,
        provider: P,
    ) -> RequestBuilder<crate::MissingUserMessage> {
        RequestBuilder::new(Some(self.clone()), provider)
    }

    pub fn cancel(&self, run_id: impl Into<crate::RunId>) -> bool {
        self.inner
            .active
            .lock()
            .ok()
            .and_then(|active_runs| active_runs.get(&run_id.into()).cloned())
            .map(|stop_handle| {
                stop_handle.stop();
                true
            })
            .unwrap_or(false)
    }

    pub fn resolve_interaction(
        &self,
        interaction_id: InteractionId,
        reply: InteractionReply,
    ) -> Result<(), LlmError> {
        self.inner
            .interactions
            .resolve_interaction(interaction_id, reply)
    }

    pub(crate) fn next_run_id(&self) -> crate::RunId {
        crate::RunId(self.inner.next_run_id.fetch_add(1, Ordering::SeqCst))
    }

    pub(crate) fn register_run(
        &self,
        run_id: crate::RunId,
        stop_handle: StopHandle,
    ) -> Result<(), LlmError> {
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

    pub(crate) fn provider_registration(
        &self,
        provider: &ProviderId,
    ) -> Result<&ProviderRegistration, LlmError> {
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
