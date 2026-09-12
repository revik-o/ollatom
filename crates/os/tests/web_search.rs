use os::{WebSearchError, WebSearchResponse, web_search};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::{self, JoinHandle},
};

#[test]
fn html_filter_removes_markup_and_code() {
    let response = WebSearchResponse {
        input: "query".into(),
        target_url: "https://example.com".into(),
        status_code: Some(200),
        body: "<html><script>ignore()</script><body>Hello &amp; <b>world</b></body></html>".into(),
        error: None,
    };
    assert_eq!(response.filter_html_code().unwrap(), "Hello & world");
}

#[test]
fn failed_status_is_preserved_and_rejected_by_filter() {
    let response = WebSearchResponse {
        input: "query".into(),
        target_url: "https://example.com".into(),
        status_code: Some(503),
        body: "unavailable".into(),
        error: Some(WebSearchError::HttpStatus(503)),
    };
    assert_eq!(response.status_code(), Some(503));
    assert!(!response.is_success());
    assert_eq!(
        response.filter_html_code(),
        Err(WebSearchError::HttpStatus(503))
    );
}

#[tokio::test]
async fn response_size_is_rejected_before_the_declared_body_is_buffered() {
    let oversized_body_length = 8 * 1024 * 1024 + 1;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {oversized_body_length}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes();
    let (target_url, server_thread) = serve_single_http_response(response);
    let search_response = web_search(target_url).await;
    server_thread.join().expect("HTTP server should finish");

    assert_eq!(
        search_response.status_code(),
        Some(200),
        "{search_response:?}"
    );
    assert_eq!(
        search_response.error(),
        Some(&WebSearchError::ResponseTooLarge)
    );
    assert!(search_response.body.is_empty());
}

#[tokio::test]
async fn redirect_is_reported_without_following_an_unauthorized_host() {
    let response = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (target_url, server_thread) = serve_single_http_response(response);
    let search_response = web_search(target_url).await;
    server_thread.join().expect("HTTP server should finish");

    assert_eq!(
        search_response.status_code(),
        Some(302),
        "{search_response:?}"
    );
    assert_eq!(
        search_response.error(),
        Some(&WebSearchError::HttpStatus(302))
    );
}

#[tokio::test]
async fn response_body_failure_preserves_the_http_status() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort".to_vec();
    let (target_url, server_thread) = serve_single_http_response(response);
    let search_response = web_search(target_url).await;
    server_thread.join().expect("HTTP server should finish");

    assert_eq!(
        search_response.status_code(),
        Some(200),
        "{search_response:?}"
    );
    assert!(matches!(
        search_response.error(),
        Some(WebSearchError::ResponseBody(_))
    ));
}

#[tokio::test]
async fn explicit_url_rejects_credentials_and_unsupported_schemes() {
    let credential_response = web_search("https://user:secret@example.com").await;
    let unsupported_scheme_response = web_search("ftp://example.com/resource").await;

    assert!(matches!(
        credential_response.error(),
        Some(WebSearchError::InvalidUrl(_))
    ));
    assert!(matches!(
        unsupported_scheme_response.error(),
        Some(WebSearchError::InvalidUrl(_))
    ));
}

fn serve_single_http_response(response: Vec<u8>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("HTTP listener should bind");
    let server_address = listener
        .local_addr()
        .expect("HTTP listener address should be available");
    let server_thread = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("HTTP connection should arrive");
        read_http_request_headers(&mut connection).expect("HTTP request should read");
        connection
            .write_all(&response)
            .expect("HTTP response should write");
    });
    (format!("http://{server_address}"), server_thread)
}

fn read_http_request_headers(connection: &mut TcpStream) -> std::io::Result<()> {
    let mut request_headers = Vec::new();
    let mut request_chunk = [0_u8; 1024];

    loop {
        let bytes_read = connection.read(&mut request_chunk)?;

        if bytes_read == 0 {
            return Ok(());
        }
        request_headers.extend_from_slice(&request_chunk[..bytes_read]);

        if request_headers
            .windows(4)
            .any(|window| window == b"\r\n\r\n")
        {
            return Ok(());
        }
    }
}
