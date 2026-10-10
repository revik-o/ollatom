use super::{WebSearchError, WebSearchResponse, build_search_target_url, decode_response_body};
use reqwest::{Client, Response, header::CONTENT_TYPE, redirect::Policy};
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};
use url::Url;

const MAXIMUM_RESPONSE_SIZE_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const GOOGLE_SEARCH_HOST: &str = "www.google.com";

pub(super) async fn web_search(search_query: impl Into<String>) -> WebSearchResponse {
    let search_query = search_query.into();

    let target_url = match build_search_target_url(&search_query) {
        Ok(target_url) => target_url,
        Err(error) => return response_with_error(search_query, None, None, error),
    };

    let validated_addresses = match resolve_validated_search_addresses().await {
        Ok(validated_addresses) => validated_addresses,
        Err(error) => {
            return response_with_error(search_query, Some(target_url), None, error);
        }
    };

    let client = match build_pinned_client(&validated_addresses) {
        Ok(client) => client,
        Err(error) => {
            return response_with_error(search_query, Some(target_url), None, error);
        }
    };

    let response = match client.get(target_url.clone()).send().await {
        Ok(response) => response,
        Err(error) => {
            return response_with_error(
                search_query,
                Some(target_url),
                None,
                WebSearchError::Request(error.to_string()),
            );
        }
    };

    let status_code = response.status().as_u16();

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let response_body = match read_bounded_response_body(response).await {
        Ok(response_body) => response_body,
        Err(error) => {
            return response_with_error(search_query, Some(target_url), Some(status_code), error);
        }
    };

    let decoded_body = match decode_response_body(&response_body, content_type.as_deref()) {
        Ok(decoded_body) => decoded_body,
        Err(error) => {
            return response_with_error(search_query, Some(target_url), Some(status_code), error);
        }
    };

    let error =
        (!(200..300).contains(&status_code)).then_some(WebSearchError::HttpStatus(status_code));

    WebSearchResponse {
        input: search_query,
        target_url: target_url.to_string(),
        status_code: Some(status_code),
        body: decoded_body,
        error,
    }
}

async fn resolve_validated_search_addresses() -> Result<Vec<SocketAddr>, WebSearchError> {
    let resolved_addresses = tokio::net::lookup_host((GOOGLE_SEARCH_HOST, 443))
        .await
        .map_err(|error| WebSearchError::HostResolution(error.to_string()))?;

    let mut validated_addresses = BTreeSet::new();

    for resolved_address in resolved_addresses {
        if !is_global_address(resolved_address.ip()) {
            return Err(WebSearchError::NonGlobalAddress);
        }

        validated_addresses.insert(resolved_address);
    }

    if validated_addresses.is_empty() {
        return Err(WebSearchError::HostResolution(
            "search host returned no addresses".into(),
        ));
    }

    Ok(validated_addresses.into_iter().collect())
}

fn is_global_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_global_ipv4_address(address),
        IpAddr::V6(address) => is_global_ipv6_address(address),
    }
}

fn is_global_ipv4_address(address: Ipv4Addr) -> bool {
    let octets = address.octets();

    !(address.is_unspecified()
        || address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_documentation()
        || octets[0] == 0
        || octets[0] >= 240
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && matches!(octets[1], 18 | 19)))
}

fn is_global_ipv6_address(address: Ipv6Addr) -> bool {
    if let Some(mapped_address) = address.to_ipv4_mapped() {
        return is_global_ipv4_address(mapped_address);
    }

    let segments = address.segments();

    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn build_pinned_client(validated_addresses: &[SocketAddr]) -> Result<Client, WebSearchError> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .user_agent("ollatom/0.0.1")
        .resolve_to_addrs(GOOGLE_SEARCH_HOST, validated_addresses)
        .build()
        .map_err(|error| WebSearchError::Request(error.to_string()))
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
    search_query: String,
    target_url: Option<Url>,
    status_code: Option<u16>,
    error: WebSearchError,
) -> WebSearchResponse {
    WebSearchResponse {
        input: search_query,
        target_url: target_url.map_or_else(String::new, |target_url| target_url.to_string()),
        status_code,
        body: String::new(),
        error: Some(error),
    }
}
