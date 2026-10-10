use super::super::metadata::{NativeModelListEntry, NativeModelListResponse};
use super::*;
use llm::TransportOptions;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct NativeServerVersionResponse {
    version: String,
}

impl OllamaProvider {
    pub(crate) async fn fetch_model_tags(
        &self,
        transport: &TransportOptions,
    ) -> Result<Vec<NativeModelListEntry>, llm::LlmError> {
        self.fetch_model_list("api/tags", transport).await
    }

    pub(crate) async fn fetch_running_models(
        &self,
        transport: &TransportOptions,
    ) -> Result<Vec<NativeModelListEntry>, llm::LlmError> {
        self.fetch_model_list("api/ps", transport).await
    }

    async fn fetch_model_list(
        &self,
        endpoint_path: &str,
        transport: &TransportOptions,
    ) -> Result<Vec<NativeModelListEntry>, llm::LlmError> {
        let response = self
            .send_discovery_request(Method::GET, endpoint_path, transport, None)
            .await?;

        Ok(Self::read_json::<NativeModelListResponse>(response, None)
            .await?
            .models)
    }

    pub(crate) async fn fetch_model_details(
        &self,
        model: &llm::ModelId,
        verbose: bool,
        transport: &TransportOptions,
    ) -> Result<Value, llm::LlmError> {
        let response = self
            .send_discovery_request(
                Method::POST,
                "api/show",
                transport,
                Some(json!({"model": model.as_str(), "verbose": verbose})),
            )
            .await?;

        validate_model_details_response(Self::read_json::<Value>(response, Some(model)).await?)
    }

    pub(crate) async fn fetch_server_version(
        &self,
        transport: &TransportOptions,
    ) -> Result<String, llm::LlmError> {
        let response = self
            .send_discovery_request(Method::GET, "api/version", transport, None)
            .await?;

        Ok(
            Self::read_json::<NativeServerVersionResponse>(response, None)
                .await?
                .version,
        )
    }

    async fn send_discovery_request(
        &self,
        method: Method,
        path: &str,
        transport: &TransportOptions,
        body: Option<Value>,
    ) -> Result<Response, llm::LlmError> {
        validate_discovery_transport_options(transport)?;

        let request = if let Some(connection_timeout_milliseconds) = transport.connect_timeout_ms {
            self.request_with_connection_timeout(
                method,
                path,
                Duration::from_millis(connection_timeout_milliseconds),
            )?
        } else {
            self.request(method, path)?
        };

        let request = if let Some(body) = body {
            request.json(&body)
        } else {
            request
        };

        let request_timeout = transport
            .overall_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(self.discovery_timeout);

        self.send_request(request, None, Some(request_timeout))
            .await
    }

    pub(crate) async fn fetch_model_details_with_stop(
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
                request.json(&json!({"model": model.as_str(), "verbose": verbose})),
                Some(stop),
                None,
            )
            .await?;

        validate_model_details_response(
            Self::read_json_with_stop(response, Some(model), Some(stop)).await?,
        )
    }
}
fn validate_discovery_transport_options(transport: &TransportOptions) -> Result<(), llm::LlmError> {
    let timeouts = [
        transport.connect_timeout_ms,
        transport.stream_idle_timeout_ms,
        transport.overall_timeout_ms,
        transport.event_delivery_timeout_ms,
    ];

    if timeouts.into_iter().flatten().any(|timeout| timeout == 0) {
        return Err(llm::LlmError::InvalidRequest(
            "transport timeouts must be greater than zero".into(),
        ));
    }

    Ok(())
}
