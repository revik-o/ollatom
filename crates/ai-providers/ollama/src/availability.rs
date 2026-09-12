use super::provider::OllamaProvider;
use llm::{AvailabilityReport, AvailabilityState, LlmError, ModelId, ProviderId};
use reqwest::StatusCode;
use serde_json::Value;
use serde_json::json;

pub(super) async fn report(
    provider: &OllamaProvider,
    selected_model: Option<ModelId>,
) -> Result<AvailabilityReport, LlmError> {
    let mut availability_report =
        initial_report(provider.provider_identifier.clone(), selected_model.clone());

    if !probe_version(provider, &mut availability_report).await? {
        return Ok(availability_report);
    }

    if let Some(selected_model) = selected_model {
        probe_model(provider, &mut availability_report, &selected_model).await?;
    } else {
        availability_report.state = AvailabilityState::Ready;
    }

    Ok(availability_report)
}

async fn probe_version(
    provider: &OllamaProvider,
    availability_report: &mut AvailabilityReport,
) -> Result<bool, LlmError> {
    let version_response = match provider
        .send_request(
            provider.request(reqwest::Method::GET, "api/version")?,
            None,
            Some(provider.discovery_timeout),
        )
        .await
    {
        Ok(response) => response,
        Err(LlmError::Unavailable(message) | LlmError::Timeout(message)) => {
            availability_report.message = Some(message);
            return Ok(false);
        }
        Err(error) => return Err(error),
    };

    availability_report.endpoint = AvailabilityState::Ready;

    if status_is_unauthorized(version_response.status()) {
        set_unauthorized(availability_report);
        return Ok(false);
    }

    if !version_response.status().is_success() {
        set_unknown(
            availability_report,
            &format!("Ollama returned HTTP {}", version_response.status()),
            false,
        );
        return Ok(false);
    }

    availability_report.authentication = AvailabilityState::Ready;
    let version_data = version_response.json::<Value>().await.ok();

    if version_data
        .as_ref()
        .and_then(|value| value.get("version"))
        .and_then(Value::as_str)
        .is_none()
    {
        set_unknown(
            availability_report,
            "Ollama returned malformed version data",
            false,
        );
        return Ok(false);
    }

    Ok(true)
}

async fn probe_model(
    provider: &OllamaProvider,
    availability_report: &mut AvailabilityReport,
    selected_model: &ModelId,
) -> Result<(), LlmError> {
    let model_response = provider
        .send_request(
            provider
                .request(reqwest::Method::POST, "api/show")?
                .json(&json!({"name": selected_model.as_str()})),
            None,
            Some(provider.discovery_timeout),
        )
        .await;

    match model_response {
        Ok(response) if response.status().is_success() => {
            if response
                .json::<Value>()
                .await
                .is_ok_and(|value| value.is_object())
            {
                availability_report.state = AvailabilityState::Ready;
                availability_report.model = AvailabilityState::Ready;
            } else {
                set_unknown(
                    availability_report,
                    "Ollama returned malformed model data",
                    false,
                );
            }
        }
        Ok(response) if status_is_unauthorized(response.status()) => {
            set_unauthorized(availability_report);
            availability_report.model = AvailabilityState::Unauthorized;
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => {
            availability_report.state = AvailabilityState::ModelMissing;
            availability_report.model = AvailabilityState::ModelMissing;
        }
        Ok(response) => set_unknown(
            availability_report,
            &format!("Ollama returned HTTP {}", response.status()),
            false,
        ),
        Err(LlmError::Unavailable(message) | LlmError::Timeout(message)) => {
            set_unknown(availability_report, &message, false);
        }
        Err(error) => return Err(error),
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

fn set_unknown(
    availability_report: &mut AvailabilityReport,
    message: &str,
    endpoint_is_unknown: bool,
) {
    availability_report.state = AvailabilityState::Unknown;
    availability_report.model = AvailabilityState::Unknown;

    if endpoint_is_unknown {
        availability_report.endpoint = AvailabilityState::Unknown;
        availability_report.authentication = AvailabilityState::Unknown;
    }

    availability_report.message = Some(message.to_string());
}

fn status_is_unauthorized(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}
