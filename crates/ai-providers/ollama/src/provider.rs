use crate::{BUILT_IN_PROVIDER, DEFAULT_OLLAMA_ENDPOINT};
use reqwest::Client;
use std::{env, fmt::Formatter, time::Duration};
use url::Url;

const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct OllamaProvider {
    pub(super) endpoint: Url,
    pub(super) bearer_token: Option<String>,
    pub(super) client: Client,
    pub(super) discovery_timeout: Duration,
    pub(super) provider_identifier: llm::ProviderId,
}

impl std::fmt::Debug for OllamaProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OllamaProvider")
            .field("endpoint", &self.endpoint)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("discovery_timeout", &self.discovery_timeout)
            .field("provider_identifier", &self.provider_identifier)
            .field("client", &"[configured]")
            .finish()
    }
}

#[derive(Clone)]
#[must_use]
pub struct OllamaProviderBuilder {
    endpoint: String,
    bearer_token: Option<String>,
    discovery_timeout: Duration,
}

impl std::fmt::Debug for OllamaProviderBuilder {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OllamaProviderBuilder")
            .field("endpoint", &self.endpoint)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("discovery_timeout", &self.discovery_timeout)
            .finish()
    }
}

impl OllamaProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("the default Ollama endpoint is valid")
    }

    pub fn builder() -> OllamaProviderBuilder {
        OllamaProviderBuilder {
            endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
            bearer_token: None,
            discovery_timeout: DEFAULT_DISCOVERY_TIMEOUT,
        }
    }

    pub fn from_environment() -> Result<Self, llm::LlmError> {
        let mut builder = Self::builder();

        if let Ok(endpoint) = env::var("OLLAMA_HOST") {
            builder.endpoint = endpoint;
        }

        builder.bearer_token = env::var("OLLAMA_API_KEY")
            .ok()
            .filter(|value| !value.is_empty());
        builder.build()
    }

    #[must_use]
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaProviderBuilder {
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.bearer_token = Some(bearer_token.into());
        self
    }

    pub fn discovery_timeout(mut self, discovery_timeout: Duration) -> Self {
        self.discovery_timeout = discovery_timeout;
        self
    }

    pub fn build(self) -> Result<OllamaProvider, llm::LlmError> {
        if self.discovery_timeout.is_zero() {
            return Err(llm::LlmError::InvalidRequest(
                "discovery timeout must be greater than zero".into(),
            ));
        }

        let endpoint = super::transport::normalize_endpoint(self.endpoint)?;
        let client = Client::builder()
            .connect_timeout(self.discovery_timeout)
            .build()
            .map_err(|error| llm::LlmError::InvalidRequest(error.to_string()))?;

        Ok(OllamaProvider {
            endpoint,
            bearer_token: self.bearer_token.filter(|value| !value.is_empty()),
            client,
            discovery_timeout: self.discovery_timeout,
            provider_identifier: llm::ProviderId::from(BUILT_IN_PROVIDER),
        })
    }
}
