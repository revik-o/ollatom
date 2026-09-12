use reqwest::{Client, Response, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};
use thiserror::Error;
use url::Url;

const MAXIMUM_RESPONSE_SIZE_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const GOOGLE_SEARCH_URL: &str = "https://www.google.com/search";

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSearchError {
    #[error("search input must not be empty")]
    EmptyInput,
    #[error("search URL is invalid: {0}")]
    InvalidUrl(String),
    #[error("web request failed: {0}")]
    Request(String),
    #[error("web response body failed: {0}")]
    ResponseBody(String),
    #[error("web response exceeded the maximum supported size")]
    ResponseTooLarge,
    #[error("web request returned HTTP status {0}")]
    HttpStatus(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct WebSearchResponse {
    pub input: String,
    pub target_url: String,
    pub status_code: Option<u16>,
    pub body: String,
    pub error: Option<WebSearchError>,
}

pub async fn web_search(search_input: impl Into<String>) -> WebSearchResponse {
    let search_input = search_input.into();
    let target_url = match build_target_url(&search_input) {
        Ok(target_url) => target_url,
        Err(error) => {
            return response_with_error(search_input, None, None, error);
        }
    };
    let client = match shared_web_client() {
        Ok(client) => client,
        Err(error) => {
            return response_with_error(search_input, Some(target_url), None, error);
        }
    };
    let response = match client.get(target_url.clone()).send().await {
        Ok(response) => response,
        Err(error) => {
            return response_with_error(
                search_input,
                Some(target_url),
                None,
                WebSearchError::Request(error.to_string()),
            );
        }
    };
    let status_code = response.status().as_u16();
    let response_body = match read_bounded_response_body(response).await {
        Ok(response_body) => String::from_utf8_lossy(&response_body).into_owned(),
        Err(error) => {
            return response_with_error(search_input, Some(target_url), Some(status_code), error);
        }
    };
    let error =
        (!((200..300).contains(&status_code))).then_some(WebSearchError::HttpStatus(status_code));

    WebSearchResponse {
        input: search_input,
        target_url: target_url.to_string(),
        status_code: Some(status_code),
        body: response_body,
        error,
    }
}

impl WebSearchResponse {
    #[must_use]
    pub fn status_code(&self) -> Option<u16> {
        self.status_code
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.error.is_none()
            && self
                .status_code
                .is_some_and(|status| (200..300).contains(&status))
    }

    #[must_use]
    pub fn error(&self) -> Option<&WebSearchError> {
        self.error.as_ref()
    }

    pub fn filter_html_code(&self) -> Result<String, WebSearchError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }

        Ok(super::web_search_html::strip_html(&self.body))
    }
}

pub fn search_target_host(search_input: &str) -> Result<String, WebSearchError> {
    build_target_url(search_input)?
        .host_str()
        .map(str::to_owned)
        .ok_or_else(|| WebSearchError::InvalidUrl("search target has no host".into()))
}

fn build_target_url(search_input: &str) -> Result<Url, WebSearchError> {
    let search_input = search_input.trim();

    if search_input.is_empty() {
        return Err(WebSearchError::EmptyInput);
    }

    if search_input.contains("://") {
        let target_url = Url::parse(search_input)
            .map_err(|error| WebSearchError::InvalidUrl(error.to_string()))?;
        validate_explicit_target_url(&target_url)?;
        return Ok(target_url);
    }

    let mut target_url = Url::parse(GOOGLE_SEARCH_URL)
        .map_err(|error| WebSearchError::InvalidUrl(error.to_string()))?;

    target_url.query_pairs_mut().append_pair("q", search_input);

    Ok(target_url)
}

fn validate_explicit_target_url(target_url: &Url) -> Result<(), WebSearchError> {
    if !matches!(target_url.scheme(), "http" | "https") {
        return Err(WebSearchError::InvalidUrl(format!(
            "unsupported URL scheme: {}",
            target_url.scheme()
        )));
    }

    if target_url.host_str().is_none() {
        return Err(WebSearchError::InvalidUrl(
            "web URL must contain a host".into(),
        ));
    }

    if !target_url.username().is_empty() || target_url.password().is_some() {
        return Err(WebSearchError::InvalidUrl(
            "web URL credentials are not supported".into(),
        ));
    }

    Ok(())
}

fn shared_web_client() -> Result<&'static Client, WebSearchError> {
    static WEB_CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();

    match WEB_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .user_agent("ollatom/0.0.1")
            .build()
            .map_err(|error| error.to_string())
    }) {
        Ok(client) => Ok(client),
        Err(error) => Err(WebSearchError::Request(error.clone())),
    }
}

async fn read_bounded_response_body(mut response: Response) -> Result<Vec<u8>, WebSearchError> {
    if response.content_length().is_some_and(|content_length| {
        content_length > u64::try_from(MAXIMUM_RESPONSE_SIZE_BYTES).unwrap_or(u64::MAX)
    }) {
        return Err(WebSearchError::ResponseTooLarge);
    }

    let initial_capacity = response
        .content_length()
        .and_then(|content_length| usize::try_from(content_length).ok())
        .unwrap_or_default()
        .min(MAXIMUM_RESPONSE_SIZE_BYTES);
    let mut response_body = Vec::with_capacity(initial_capacity);

    loop {
        let response_chunk = response
            .chunk()
            .await
            .map_err(|error| WebSearchError::ResponseBody(error.to_string()))?;
        let Some(response_chunk) = response_chunk else {
            return Ok(response_body);
        };
        let remaining_capacity = MAXIMUM_RESPONSE_SIZE_BYTES.saturating_sub(response_body.len());

        if response_chunk.len() > remaining_capacity {
            return Err(WebSearchError::ResponseTooLarge);
        }

        response_body.extend_from_slice(&response_chunk);
    }
}

fn response_with_error(
    search_input: String,
    target_url: Option<Url>,
    status_code: Option<u16>,
    error: WebSearchError,
) -> WebSearchResponse {
    WebSearchResponse {
        input: search_input,
        target_url: target_url.map_or_else(String::new, |target_url| target_url.to_string()),
        status_code,
        body: String::new(),
        error: Some(error),
    }
}
