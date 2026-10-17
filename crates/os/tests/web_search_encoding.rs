#![allow(dead_code)]

#[path = "../src/web_search.rs"]
mod web_search;
#[path = "../src/web_search_html.rs"]
mod web_search_html;

use web_search::{WebSearchError, build_search_target_url, decode_response_body};

#[test]
fn explicit_url_text_is_encoded_as_a_search_query() {
    let target_url = build_search_target_url("http://127.0.0.1/private").unwrap();

    assert_eq!(target_url.host_str(), Some("www.google.com"));
    assert_eq!(
        target_url
            .query_pairs()
            .find(|(name, _)| name == "q")
            .map(|(_, value)| value.into_owned()),
        Some("http://127.0.0.1/private".into())
    );
}

#[test]
fn response_body_uses_declared_charset() {
    let decoded_body = decode_response_body(&[0x63, 0x61, 0x66, 0xE9], Some("text/html; charset=windows-1252")).unwrap();

    assert_eq!(decoded_body, "café");
}

#[test]
fn unsupported_charset_is_rejected() {
    assert_eq!(
        decode_response_body(b"body", Some("text/html; charset=invalid-charset")),
        Err(WebSearchError::UnsupportedCharset("invalid-charset".into()))
    );
}
