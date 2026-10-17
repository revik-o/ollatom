use os::{WebSearchError, WebSearchResponse, search_target_host};

#[test]
fn html_filter_removes_markup_and_executable_content() {
    let response = successful_response(
        "<html><script>ignore()</script><style>ignore</style><body>Hello &amp; <b>world</b></body></html>",
    );

    assert_eq!(response.filter_html_code().unwrap(), "Hello & world");
}

#[test]
fn html_filter_preserves_literal_less_than_text() {
    let response = successful_response("<p>one &lt; two</p><p>three < four</p>");

    assert_eq!(response.filter_html_code().unwrap(), "one < two three < four");
}

#[test]
fn malformed_html_is_normalized_without_panicking() {
    let response = successful_response("<main><p>first<p>second &amp; third");

    assert_eq!(response.filter_html_code().unwrap(), "first second & third");
}

#[test]
fn failed_status_is_preserved_and_rejected_by_filter() {
    let response = WebSearchResponse {
        input: "query".into(),
        target_url: "https://www.google.com/search?q=query".into(),
        status_code: Some(503),
        body: "unavailable".into(),
        error: Some(WebSearchError::HttpStatus(503)),
    };

    assert_eq!(response.status_code(), Some(503));
    assert!(!response.is_success());
    assert_eq!(response.filter_html_code(), Err(WebSearchError::HttpStatus(503)));
}

#[test]
fn every_nonempty_input_uses_the_fixed_search_host() {
    assert_eq!(search_target_host("ordinary query").unwrap(), "www.google.com");
    assert_eq!(
        search_target_host("http://127.0.0.1/private").unwrap(),
        "www.google.com"
    );
    assert_eq!(search_target_host(" "), Err(WebSearchError::EmptyInput));
}

fn successful_response(body: &str) -> WebSearchResponse {
    WebSearchResponse {
        input: "query".into(),
        target_url: "https://www.google.com/search?q=query".into(),
        status_code: Some(200),
        body: body.into(),
        error: None,
    }
}
