use super::{availability, metadata::provider_capabilities, models, provider::OllamaProvider, run};
use llm::{
    AvailabilityReport, BoxFuture, LlmError, LlmProvider, ModelId, ModelInfo, ModelScope,
    ProviderCapabilities, ProviderRunHost, ProviderRunOutcome, ProviderRunRequest, StopToken,
    TransportOptions,
};
use std::sync::Arc;

impl LlmProvider for OllamaProvider {
    fn id(&self) -> &llm::ProviderId {
        &self.provider_identifier
    }

    fn capabilities(&self) -> ProviderCapabilities {
        provider_capabilities()
    }

    fn availability<'a>(
        &'a self,
        model: Option<&'a ModelId>,
        transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<AvailabilityReport, LlmError>> {
        Box::pin(availability::report(self, model.cloned(), transport))
    }

    fn list_models<'a>(
        &'a self,
        scope: ModelScope,
        transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<Vec<ModelInfo>, LlmError>> {
        Box::pin(models::list(self, scope, transport))
    }

    fn model_info<'a>(
        &'a self,
        selected: &'a ModelId,
        transport: &'a TransportOptions,
    ) -> BoxFuture<'a, Result<ModelInfo, LlmError>> {
        Box::pin(models::info(self, selected.clone(), transport))
    }

    fn run(
        &self,
        request: ProviderRunRequest,
        host: Arc<dyn ProviderRunHost>,
        stop: StopToken,
    ) -> BoxFuture<'_, Result<ProviderRunOutcome, LlmError>> {
        Box::pin(run::model(self.clone(), request, host, stop))
    }
}
