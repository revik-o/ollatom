use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct ScriptedResponse {
    status_code: u16,
    reason_phrase: &'static str,
    content_type: &'static str,
    body: String,
}

impl ScriptedResponse {
    pub fn json(status_code: u16, reason_phrase: &'static str, body: Value) -> Self {
        Self {
            status_code,
            reason_phrase,
            content_type: "application/json",
            body: body.to_string(),
        }
    }

    pub fn stream(body: impl Into<String>) -> Self {
        Self {
            status_code: 200,
            reason_phrase: "OK",
            content_type: "application/x-ndjson",
            body: body.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CapturedRequest {
    pub request_line: String,
    pub headers: String,
    pub body: String,
}

pub struct ScriptedServer {
    endpoint: String,
    captured_requests: Arc<Mutex<Vec<CapturedRequest>>>,
    server_thread: Option<JoinHandle<()>>,
}

impl ScriptedServer {
    pub fn start(scripted_responses: Vec<ScriptedResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let endpoint = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("test server address should be available")
        );
        let captured_requests = Arc::new(Mutex::new(Vec::new()));
        let server_captured_requests = captured_requests.clone();
        let server_thread = thread::spawn(move || {
            for scripted_response in scripted_responses {
                let (mut connection, _) = listener.accept().expect("test server should accept");
                connection
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("test server timeout should configure");
                let captured_request = read_request(&mut connection);
                server_captured_requests
                    .lock()
                    .expect("captured request lock should be available")
                    .push(captured_request);
                write_response(&mut connection, scripted_response);
            }
        });
        Self {
            endpoint,
            captured_requests,
            server_thread: Some(server_thread),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn finish(mut self) -> Vec<CapturedRequest> {
        self.server_thread
            .take()
            .expect("test server thread should exist")
            .join()
            .expect("test server thread should finish");
        Arc::try_unwrap(self.captured_requests)
            .expect("captured requests should have one owner")
            .into_inner()
            .expect("captured request lock should be available")
    }
}

fn read_request(connection: &mut TcpStream) -> CapturedRequest {
    let mut request_bytes = Vec::new();
    let header_end = loop {
        let mut request_buffer = [0_u8; 4096];
        let bytes_read = connection
            .read(&mut request_buffer)
            .expect("test request should be readable");
        assert!(bytes_read > 0, "test request ended before its headers");
        request_bytes.extend_from_slice(&request_buffer[..bytes_read]);

        if let Some(header_end) = find_header_end(&request_bytes) {
            break header_end;
        }
    };
    let headers = String::from_utf8(request_bytes[..header_end].to_vec())
        .expect("test request headers should be UTF-8");
    let content_length = content_length(&headers);

    while request_bytes.len() < header_end + content_length {
        let mut request_buffer = [0_u8; 4096];
        let bytes_read = connection
            .read(&mut request_buffer)
            .expect("test request body should be readable");
        assert!(bytes_read > 0, "test request ended before its body");
        request_bytes.extend_from_slice(&request_buffer[..bytes_read]);
    }
    CapturedRequest {
        request_line: headers.lines().next().unwrap_or_default().to_owned(),
        headers,
        body: String::from_utf8(request_bytes[header_end..header_end + content_length].to_vec())
            .expect("test request body should be UTF-8"),
    }
}

fn find_header_end(request_bytes: &[u8]) -> Option<usize> {
    request_bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|header_line| {
            let (header_name, header_value) = header_line.split_once(':')?;
            header_name.eq_ignore_ascii_case("content-length").then(|| {
                header_value
                    .trim()
                    .parse()
                    .expect("content length should parse")
            })
        })
        .unwrap_or(0)
}

fn write_response(connection: &mut TcpStream, scripted_response: ScriptedResponse) {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        scripted_response.status_code,
        scripted_response.reason_phrase,
        scripted_response.content_type,
        scripted_response.body.len(),
        scripted_response.body
    );
    connection
        .write_all(response.as_bytes())
        .expect("test response should be writable");
}
