use super::{availability, metadata::provider_capabilities, models, provider::OllamaProvider, run};
use llm::{
    AvailabilityReport, BoxFuture, LlmError, LlmProvider, ModelId, ModelInfo, ModelScope,
    ProviderCapabilities, ProviderRunHost, ProviderRunOutcome, ProviderRunRequest, StopToken,
};
use std::sync::Arc;

impl LlmProvider for OllamaProvider {
    fn id(&self) -> &llm::ProviderId {
        &self.provider_identifier
    }

    fn capabilities(&self) -> ProviderCapabilities {
        provider_capabilities()
    }

    fn availability(
        &self,
        model: Option<&ModelId>,
    ) -> BoxFuture<'_, Result<AvailabilityReport, LlmError>> {
        Box::pin(availability::report(self, model.cloned()))
    }

    fn list_models(&self, scope: ModelScope) -> BoxFuture<'_, Result<Vec<ModelInfo>, LlmError>> {
        Box::pin(models::list(self, scope))
    }

    fn model_info(&self, selected: &ModelId) -> BoxFuture<'_, Result<ModelInfo, LlmError>> {
        Box::pin(models::info(self, selected.clone()))
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
