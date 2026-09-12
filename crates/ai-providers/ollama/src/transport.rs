use super::metadata::{
    NativeModelTag, NativeModelTagsResponse, NativeRunningModel, NativeRunningModelsResponse,
};
use super::provider::OllamaProvider;
use reqwest::{Client, Method, Response, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use url::Url;

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
        let client = Client::builder()
            .connect_timeout(timeout)
            .build()
            .map_err(|error| llm::LlmError::InvalidRequest(error.to_string()))?;

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
            tokio::select! {
                () = stop_token.cancelled() => Err(llm::LlmError::Cancelled),
                result = send_request_future => result.map_err(network_error),
            }
        } else {
            send_request_future.await.map_err(network_error)
        }
    }

    pub(super) async fn read_json<T: for<'de> Deserialize<'de>>(
        response: Response,
        requested_model: Option<&llm::ModelId>,
    ) -> Result<T, llm::LlmError> {
        let status = response.status();

        if !status.is_success() {
            return Err(response_error(status, requested_model));
        }

        response.json().await.map_err(protocol_error)
    }

    pub(super) async fn fetch_model_tags(&self) -> Result<Vec<NativeModelTag>, llm::LlmError> {
        let response = self
            .send_request(
                self.request(Method::GET, "api/tags")?,
                None,
                Some(self.discovery_timeout),
            )
            .await?;

        Ok(Self::read_json::<NativeModelTagsResponse>(response, None)
            .await?
            .models)
    }

    pub(super) async fn fetch_running_models(
        &self,
    ) -> Result<Vec<NativeRunningModel>, llm::LlmError> {
        let response = self
            .send_request(
                self.request(Method::GET, "api/ps")?,
                None,
                Some(self.discovery_timeout),
            )
            .await?;

        Ok(
            Self::read_json::<NativeRunningModelsResponse>(response, None)
                .await?
                .models,
        )
    }

    pub(super) async fn fetch_model_details(
        &self,
        model: &llm::ModelId,
        verbose: bool,
    ) -> Result<Value, llm::LlmError> {
        let response = self
            .send_request(
                self.request(Method::POST, "api/show")?
                    .json(&json!({"name": model.as_str(), "verbose": verbose})),
                None,
                Some(self.discovery_timeout),
            )
            .await?;

        validate_model_details_response(Self::read_json::<Value>(response, Some(model)).await?)
    }

    pub(super) async fn fetch_model_details_with_stop(
        &self,
        model: &llm::ModelId,
        verbose: bool,
        stop: &llm::StopToken,
        connection_timeout: Option<Duration>,
    ) -> Result<Value, llm::LlmError> {
        let request = if let Some(timeout) = connection_timeout {
            self.request_with_connection_timeout(Method::POST, "api/show", timeout)?
        } else {
            self.request(Method::POST, "api/show")?
        };

        let response = self
            .send_request(
                request.json(&json!({"name": model.as_str(), "verbose": verbose})),
                Some(stop),
                None,
            )
            .await?;

        validate_model_details_response(Self::read_json::<Value>(response, Some(model)).await?)
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
        return llm::LlmError::Unavailable("Ollama authentication failed".into());
    }

    llm::LlmError::Provider(format!("Ollama returned HTTP {status}"))
}

pub(super) fn network_error(error: reqwest::Error) -> llm::LlmError {
    if error.is_timeout() {
        llm::LlmError::Timeout("Ollama request timed out".into())
    } else {
        llm::LlmError::Unavailable("Ollama daemon is unreachable".into())
    }
}

fn protocol_error(_: reqwest::Error) -> llm::LlmError {
    llm::LlmError::ProviderProtocol("invalid Ollama response".into())
}

fn validate_model_details_response(model_details: Value) -> Result<Value, llm::LlmError> {
    if model_details.is_object() {
        Ok(model_details)
    } else {
        Err(llm::LlmError::ProviderProtocol(
            "invalid Ollama model response".into(),
        ))
    }
}
