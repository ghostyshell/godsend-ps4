//! Functional test for the loopback HTTP transport: initialize, list tools,
//! and a notification, exactly as an MCP client would send them.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn post(port: u16, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status: u16 = response
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn speaks_mcp_over_http() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let _ = godsend_ps4_mcp::serve_http_listener(listener);
    });

    let (status, body) = post(
        port,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"name\":\"godsend-ps4\""), "body: {body}");

    let (status, body) = post(port, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert_eq!(status, 200);
    assert!(body.contains("\"ps4_library\""), "body: {body}");
    assert!(body.contains("\"ps4_pad_status\""));

    // A notification is accepted with no JSON-RPC response.
    let (status, body) = post(
        port,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!(status, 202);
    assert!(body.is_empty());

    // A GET is rejected.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .write_all(b"GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 405"));
}
