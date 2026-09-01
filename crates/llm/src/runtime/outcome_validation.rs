use crate::{LlmError, ModelId, ProviderId, ProviderRunOutcome};

pub(super) fn validate_identity(
    outcome: &ProviderRunOutcome,
    provider: &ProviderId,
    model: &ModelId,
) -> Result<(), LlmError> {
    let (actual_provider, actual_model) = match outcome {
        ProviderRunOutcome::Completed(response) => (&response.provider, &response.model),
        ProviderRunOutcome::Cancelled(response) => (&response.provider, &response.model),
    };

    if actual_provider != provider || actual_model != model {
        return Err(LlmError::ProviderProtocol(format!(
            "provider returned identity {actual_provider}/{actual_model}, expected {provider}/{model}"
        )));
    }

    Ok(())
}
