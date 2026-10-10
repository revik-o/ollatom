use super::provider::OllamaProvider;
use super::transport::is_authentication_failure;
use llm::{AvailabilityReport, AvailabilityState, LlmError, ModelId, ProviderId, TransportOptions};

pub(super) async fn report(
    provider: &OllamaProvider,
    selected_model: Option<ModelId>,
    transport: &TransportOptions,
) -> Result<AvailabilityReport, LlmError> {
    let mut availability_report =
        initial_report(provider.provider_identifier.clone(), selected_model.clone());

    if !probe_version(provider, &mut availability_report, transport).await? {
        return Ok(availability_report);
    }

    if let Some(selected_model) = selected_model {
        probe_model(
            provider,
            &mut availability_report,
            &selected_model,
            transport,
        )
        .await?;
    } else {
        availability_report.state = AvailabilityState::Ready;
    }

    Ok(availability_report)
}

async fn probe_version(
    provider: &OllamaProvider,
    availability_report: &mut AvailabilityReport,
    transport: &TransportOptions,
) -> Result<bool, LlmError> {
    match provider.fetch_server_version(transport).await {
        Ok(_) => {
            availability_report.endpoint = AvailabilityState::Ready;
            availability_report.authentication = AvailabilityState::Ready;
            Ok(true)
        }
        Err(error) if is_authentication_failure(&error) => {
            set_unauthorized(availability_report);
            Ok(false)
        }
        Err(LlmError::Unavailable(message) | LlmError::Timeout(message)) => {
            availability_report.message = Some(message);
            Ok(false)
        }
        Err(error) => {
            availability_report.endpoint = AvailabilityState::Ready;
            availability_report.authentication = AvailabilityState::Ready;
            set_unknown(availability_report, &error.to_string());
            Ok(false)
        }
    }
}

async fn probe_model(
    provider: &OllamaProvider,
    availability_report: &mut AvailabilityReport,
    selected_model: &ModelId,
    transport: &TransportOptions,
) -> Result<(), LlmError> {
    match provider
        .fetch_model_details(selected_model, false, transport)
        .await
    {
        Ok(_) => {
            availability_report.state = AvailabilityState::Ready;
            availability_report.model = AvailabilityState::Ready;
        }
        Err(LlmError::ModelNotFound(_)) => {
            availability_report.state = AvailabilityState::ModelMissing;
            availability_report.model = AvailabilityState::ModelMissing;
        }
        Err(error) if is_authentication_failure(&error) => {
            set_unauthorized(availability_report);
            availability_report.model = AvailabilityState::Unauthorized;
        }
        Err(LlmError::Unavailable(message) | LlmError::Timeout(message)) => {
            set_unknown(availability_report, &message);
        }
        Err(error) => set_unknown(availability_report, &error.to_string()),
    }

    Ok(())
}

fn initial_report(
    provider_identifier: ProviderId,
    selected_model: Option<ModelId>,
) -> AvailabilityReport {
    AvailabilityReport {
        provider: provider_identifier,
        state: AvailabilityState::Unreachable,
        endpoint: AvailabilityState::Unreachable,
        authentication: AvailabilityState::Unknown,
        model: selected_model
            .as_ref()
            .map_or(AvailabilityState::MissingConfiguration, |_| {
                AvailabilityState::Unknown
            }),
        selected_model,
        message: None,
    }
}

fn set_unauthorized(availability_report: &mut AvailabilityReport) {
    availability_report.state = AvailabilityState::Unauthorized;
    availability_report.endpoint = AvailabilityState::Ready;
    availability_report.authentication = AvailabilityState::Unauthorized;
}

fn set_unknown(availability_report: &mut AvailabilityReport, message: &str) {
    availability_report.state = AvailabilityState::Unknown;
    availability_report.model = AvailabilityState::Unknown;
    availability_report.message = Some(message.to_string());
}
