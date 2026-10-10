use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[path = "web_search/request.rs"]
mod request;

const GOOGLE_SEARCH_URL: &str = "https://www.google.com/search";

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WebSearchError {
    #[error("search input must not be empty")]
    EmptyInput,
    #[error("search URL is invalid: {0}")]
    InvalidUrl(String),
    #[error("search host resolution failed: {0}")]
    HostResolution(String),
    #[error("search host resolved to a non-global address")]
    NonGlobalAddress,
    #[error("web request failed: {0}")]
    Request(String),
    #[error("web response body failed: {0}")]
    ResponseBody(String),
    #[error("web response exceeded the maximum supported size")]
    ResponseTooLarge,
    #[error("web response used an unsupported charset: {0}")]
    UnsupportedCharset(String),
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

pub async fn web_search(search_query: impl Into<String>) -> WebSearchResponse {
    request::web_search(search_query).await
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

pub fn search_target_host(search_query: &str) -> Result<String, WebSearchError> {
    build_search_target_url(search_query)?
        .host_str()
        .map(str::to_owned)
        .ok_or_else(|| WebSearchError::InvalidUrl("search target has no host".into()))
}

pub(super) fn build_search_target_url(search_query: &str) -> Result<Url, WebSearchError> {
    let normalized_search_query = search_query.trim();

    if normalized_search_query.is_empty() {
        return Err(WebSearchError::EmptyInput);
    }

    let mut target_url = Url::parse(GOOGLE_SEARCH_URL)
        .map_err(|error| WebSearchError::InvalidUrl(error.to_string()))?;
    target_url
        .query_pairs_mut()
        .append_pair("q", normalized_search_query);

    Ok(target_url)
}

pub(super) fn decode_response_body(
    response_body: &[u8],
    content_type: Option<&str>,
) -> Result<String, WebSearchError> {
    let charset_label = content_type.and_then(extract_charset_label);
    let encoding = match charset_label {
        Some(charset_label) => Encoding::for_label(charset_label.as_bytes())
            .ok_or_else(|| WebSearchError::UnsupportedCharset(charset_label.to_owned()))?,
        None => UTF_8,
    };
    let (decoded_body, _, _) = encoding.decode(response_body);

    Ok(decoded_body.into_owned())
}

fn extract_charset_label(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;

        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['\'', '"']))
    })
}
