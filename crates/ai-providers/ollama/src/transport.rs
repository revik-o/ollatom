use super::provider::OllamaProvider;
use reqwest::{Client, Method, Response, StatusCode, redirect::Policy};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use url::Url;

mod body;
mod discovery;
use body::read_bounded_json_body;

const MAXIMUM_JSON_RESPONSE_BYTE_COUNT: usize = 8 * 1024 * 1024;
const MAXIMUM_CACHED_CLIENT_COUNT: usize = 16;
const AUTHENTICATION_FAILURE_MESSAGE: &str = "Ollama authentication failed";

impl OllamaProvider {
    pub(super) fn request(
        &self,
        method: Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, llm::LlmError> {
        self.request_with_client(&self.client, method, path)
    }

    pub(super) fn request_with_connection_timeout(
        &self,
        method: Method,
        path: &str,
        timeout: Duration,
    ) -> Result<reqwest::RequestBuilder, llm::LlmError> {
        let client = {
            let mut clients_by_connection_timeout = self
                .clients_by_connection_timeout
                .lock()
                .map_err(|_| llm::LlmError::Provider("Ollama client cache lock failed".into()))?;

            if let Some(client) = clients_by_connection_timeout.get(&timeout) {
                client.clone()
            } else {
                let client = Client::builder()
                    .connect_timeout(timeout)
                    .redirect(Policy::none())
                    .build()
                    .map_err(|error| llm::LlmError::InvalidRequest(error.to_string()))?;

                if clients_by_connection_timeout.len() >= MAXIMUM_CACHED_CLIENT_COUNT
                    && let Some(oldest_timeout) =
                        clients_by_connection_timeout.keys().next().copied()
                {
                    clients_by_connection_timeout.remove(&oldest_timeout);
                }

                clients_by_connection_timeout.insert(timeout, client.clone());
                client
            }
        };

        self.request_with_client(&client, method, path)
    }

    fn request_with_client(
        &self,
        client: &Client,
        method: Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, llm::LlmError> {
        let url = self
            .endpoint
            .join(path)
            .map_err(|error| llm::LlmError::InvalidRequest(error.to_string()))?;
        let request_builder = client.request(method, url);

        Ok(if let Some(bearer_token) = &self.bearer_token {
            request_builder.bearer_auth(bearer_token)
        } else {
            request_builder
        })
    }

    pub(super) async fn send_request(
        &self,
        request: reqwest::RequestBuilder,
        stop: Option<&llm::StopToken>,
        request_timeout: Option<Duration>,
    ) -> Result<Response, llm::LlmError> {
        let request = if let Some(duration) = request_timeout {
            request.timeout(duration)
        } else {
            request
        };

        let send_request_future = request.send();

        if let Some(stop_token) = stop {
            match stop_token.run_until_cancelled(send_request_future).await {
                llm::ControlledFutureOutcome::Cancelled => Err(llm::LlmError::Cancelled),
                llm::ControlledFutureOutcome::TimedOut => unreachable!(),
                llm::ControlledFutureOutcome::Completed(result) => result.map_err(network_error),
            }
        } else {
            send_request_future.await.map_err(network_error)
        }
    }

    pub(super) async fn read_json<T: for<'de> Deserialize<'de>>(
        response: Response,
        requested_model: Option<&llm::ModelId>,
    ) -> Result<T, llm::LlmError> {
        Self::read_json_with_stop(response, requested_model, None).await
    }

    pub(super) async fn read_json_with_stop<T: for<'de> Deserialize<'de>>(
        response: Response,
        requested_model: Option<&llm::ModelId>,
        stop: Option<&llm::StopToken>,
    ) -> Result<T, llm::LlmError> {
        let status = response.status();

        if !status.is_success() {
            return Err(response_error(status, requested_model));
        }

        let response_body = read_bounded_json_body(response, stop).await?;

        serde_json::from_slice(&response_body)
            .map_err(|_| llm::LlmError::ProviderProtocol("invalid Ollama response".into()))
    }
}

pub(super) fn normalize_endpoint(endpoint: String) -> Result<Url, llm::LlmError> {
    let endpoint_with_scheme = if endpoint.contains("://") {
        endpoint
    } else {
        format!("http://{endpoint}")
    };

    let mut normalized_endpoint = Url::parse(&endpoint_with_scheme).map_err(|error| {
        llm::LlmError::InvalidRequest(format!("invalid Ollama endpoint: {error}"))
    })?;

    if !matches!(normalized_endpoint.scheme(), "http" | "https") {
        return Err(llm::LlmError::InvalidRequest(
            "endpoint must use http or https".into(),
        ));
    }

    if normalized_endpoint.host_str().is_none() {
        return Err(llm::LlmError::InvalidRequest(
            "endpoint must include a host".into(),
        ));
    }

    if !normalized_endpoint.username().is_empty() || normalized_endpoint.password().is_some() {
        return Err(llm::LlmError::InvalidRequest(
            "endpoint credentials must be supplied as a bearer token".into(),
        ));
    }

    normalized_endpoint.set_query(None);
    normalized_endpoint.set_fragment(None);
    ensure_trailing_path_separator(&mut normalized_endpoint);

    Ok(normalized_endpoint)
}

fn ensure_trailing_path_separator(endpoint: &mut Url) {
    if !endpoint.path().ends_with('/') {
        endpoint.set_path(&format!("{}/", endpoint.path()));
    }
}

pub(super) fn response_error(status: StatusCode, model: Option<&llm::ModelId>) -> llm::LlmError {
    if status == StatusCode::NOT_FOUND
        && let Some(model) = model
    {
        return llm::LlmError::ModelNotFound(model.clone());
    }

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return llm::LlmError::Unavailable(AUTHENTICATION_FAILURE_MESSAGE.into());
    }

    llm::LlmError::Provider(format!("Ollama returned HTTP {status}"))
}

pub(super) fn is_authentication_failure(error: &llm::LlmError) -> bool {
    matches!(
        error,
        llm::LlmError::Unavailable(message) if message == AUTHENTICATION_FAILURE_MESSAGE
    )
}

pub(super) fn network_error(error: reqwest::Error) -> llm::LlmError {
    error
        .is_timeout()
        .then_some(llm::LlmError::Timeout("Ollama request timed out".into()))
        .unwrap_or(llm::LlmError::Unavailable(
            "Ollama daemon is unreachable".into(),
        ))
}

fn protocol_error(_: reqwest::Error) -> llm::LlmError {
    llm::LlmError::ProviderProtocol("invalid Ollama response".into())
}

pub(super) fn validate_model_details_response(
    model_details: Value,
) -> Result<Value, llm::LlmError> {
    let recognized_fields = [
        "capabilities",
        "details",
        "license",
        "model_info",
        "modified_at",
        "parameters",
        "template",
    ];

    if model_details.as_object().is_some_and(|model_details| {
        recognized_fields
            .iter()
            .any(|recognized_field| model_details.contains_key(*recognized_field))
    }) {
        Ok(model_details)
    } else {
        Err(llm::LlmError::ProviderProtocol(
            "invalid Ollama model response".into(),
        ))
    }
}
