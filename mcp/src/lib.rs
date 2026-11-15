//! Model Context Protocol server for GODsend PS4.
//!
//! The same tool handler backs two transports:
//!
//! - stdio, for MCP clients that spawn `godsend-ps4-mcp`
//! - HTTP on loopback, started automatically by the desktop app
//!
//! Tools are thin wrappers over `godsend-core`, so an agent drives the same
//! FTP, installer and pad paths the UI does. Long operations run in a job and
//! block the call until they finish.

use godsend_core::config::Config;
use godsend_core::convert;
use godsend_core::device;
use godsend_core::doctor;
use godsend_core::fm;
use godsend_core::homebrew;
use godsend_core::install;
use godsend_core::library;
use godsend_core::pad;
use godsend_core::saves;
use godsend_core::sources;
use godsend_core::transfer::{JobContext, JobManager};
use godsend_core::{Error, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "godsend-ps4";

/// Handle one JSON-RPC message. Returns `None` for notifications.
pub fn handle_message(message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    if id.is_none() {
        // notifications/initialized, notifications/cancelled, ...
        return None;
    }
    let id = id.unwrap();

    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(&params),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err(RpcError::method_not_found(other)),
    };

    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": err.code, "message": err.message }
        }),
    })
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn invalid(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
        }
    }
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": requested,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": godsend_core::VERSION
        },
        "instructions": "GODsend PS4 controls a PS4 running GoldHEN over FTP, its remote package installer and the Control4Free virtual pad. Start with ps4_status, then ps4_library. Long calls (install, upload, download, convert, backup) block until the job finishes."
    })
}

fn tools() -> Value {
    let tool = |name: &str, description: &str, schema: Value| json!({ "name": name, "description": description, "inputSchema": schema });
    json!([
        tool("ps4_doctor", "Preflight checks: configured console, FTP, package installer, virtual pad, download folder, debrid.", json!({ "type": "object", "properties": {} })),
        tool("ps4_hosts", "List configured consoles with live FTP, installer and pad status.", json!({ "type": "object", "properties": {} })),
        tool("ps4_status", "Probe one console (the active one unless id is given).", json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "Host id or address" } }
        })),
        tool("ps4_library", "Scan installed titles and packages waiting in /user/pkg.", json!({ "type": "object", "properties": {} })),
        tool("ps4_install", "Install a local PKG (through the console's installer), send a payload, or unpack a zip first.", json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        })),
        tool("ps4_send_payload", "Send a payload file over the bin loader (port 9090).", json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        })),
        tool("ps4_ls", "List a directory on the console.", json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "e.g. /user/appmeta" } },
            "required": ["path"]
        })),
        tool("ps4_upload", "Upload a local file or folder to a console directory.", json!({
            "type": "object",
            "properties": {
                "local": { "type": "string" },
                "remote_dir": { "type": "string" }
            },
            "required": ["local", "remote_dir"]
        })),
        tool("ps4_download", "Download a file or folder from the console into a local directory.", json!({
            "type": "object",
            "properties": {
                "remote": { "type": "string" },
                "local_dir": { "type": "string" }
            },
            "required": ["remote", "local_dir"]
        })),
        tool("ps4_inspect", "Inspect a local PKG, payload, unpacked tree or split set.", json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        })),
        tool("ps4_convert", "Unpack a PKG, split a file for FAT32, or merge a split set.", json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "enum": ["pkg_extract", "split", "merge"] },
                "src": { "type": "string" },
                "out": { "type": "string" },
                "chunk_mb": { "type": "integer", "default": 4095 }
            },
            "required": ["kind", "src", "out"]
        })),
        tool("ps4_search", "Search the game index or Internet Archive for PS4 content.", json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "source": { "type": "string", "enum": ["gamesearch", "ia"], "default": "gamesearch" }
            },
            "required": ["query"]
        })),
        tool("ps4_homebrew", "List the homebrew catalog (packages and payloads).", json!({
            "type": "object",
            "properties": { "refresh": { "type": "boolean", "default": false } }
        })),
        tool("ps4_homebrew_install", "Download and install a homebrew entry by id.", json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        })),
        tool("ps4_saves", "List save folders under /user/home/<user>/savedata.", json!({ "type": "object", "properties": {} })),
        tool("ps4_save_backup", "Zip selected saves locally. Pass an empty list for all saves.", json!({
            "type": "object",
            "properties": {
                "title_ids": { "type": "array", "items": { "type": "string" } },
                "out": { "type": "string", "description": "Destination zip path" }
            },
            "required": ["title_ids", "out"]
        })),
        tool("ps4_pad_status", "Report the virtual pad service (Control4Free) on port 4264.", json!({ "type": "object", "properties": {} }))
    ])
}

fn call_tool(params: &Value) -> std::result::Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid("tools/call needs a name"))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match run_tool(name, &args) {
        Ok(text) => Ok(json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        })),
        Err(err) => Ok(json!({
            "content": [{ "type": "text", "text": err.to_string() }],
            "isError": true
        })),
    }
}

fn run_tool(name: &str, args: &Value) -> Result<String> {
    let config = Config::load();
    match name {
        "ps4_doctor" => Ok(to_text(&doctor::run(&config))),
        "ps4_hosts" => {
            let statuses = device::probe_all(&config.hosts);
            Ok(to_text(&json!({
                "active": config.active_host,
                "hosts": statuses
            })))
        }
        "ps4_status" => {
            let host = match arg_str(args, "id") {
                Some(id) => config
                    .hosts
                    .iter()
                    .find(|h| h.id == id || h.addr == id)
                    .cloned()
                    .ok_or_else(|| Error::NotFound(format!("no host {id}")))?,
                None => config.require_active()?,
            };
            Ok(to_text(&device::probe(&host)))
        }
        "ps4_library" => Ok(to_text(&library::scan(&config.require_active()?)?)),
        "ps4_install" => {
            let path = arg_required(args, "path")?;
            let host = config.require_active()?;
            run_job("install", move |ctx| {
                let report = install::install_local(&host, &PathBuf::from(&path), &config, ctx)?;
                ctx.message(report.message);
                Ok(())
            })
        }
        "ps4_send_payload" => {
            let path = arg_required(args, "path")?;
            install::send_payload(&config.require_active()?, &PathBuf::from(&path))
        }
        "ps4_ls" => {
            let path = arg_required(args, "path")?;
            Ok(to_text(&fm::remote_list(&config.require_active()?, &path)?))
        }
        "ps4_upload" => {
            let local = arg_required(args, "local")?;
            let remote = arg_required(args, "remote_dir")?;
            let host = config.require_active()?;
            let split_bytes = config.split_bytes;
            run_job("upload", move |ctx| {
                let path = fm::upload(
                    &host,
                    &PathBuf::from(&local),
                    &remote,
                    split_bytes,
                    ctx,
                )?;
                ctx.message(format!("uploaded to {path}"));
                Ok(())
            })
        }
        "ps4_download" => {
            let remote = arg_required(args, "remote")?;
            let local = arg_required(args, "local_dir")?;
            let host = config.require_active()?;
            run_job("download", move |ctx| {
                let path = fm::download(&host, &remote, &PathBuf::from(&local), ctx)?;
                ctx.message(format!("saved to {}", path.display()));
                Ok(())
            })
        }
        "ps4_inspect" => Ok(to_text(&convert::inspect(&PathBuf::from(arg_required(
            args, "path",
        )?))?)),
        "ps4_convert" => {
            let kind = arg_required(args, "kind")?;
            let src = PathBuf::from(arg_required(args, "src")?);
            let out = PathBuf::from(arg_required(args, "out")?);
            let chunk = args
                .get("chunk_mb")
                .and_then(|v| v.as_u64())
                .unwrap_or(4095);
            run_job("convert", move |ctx| match kind.as_str() {
                "pkg_extract" => convert::pkg_extract(&src, &out, ctx).map(|_| ()),
                "split" => convert::split_file(&src, chunk * 1024 * 1024, ctx).map(|_| ()),
                "merge" => convert::merge_file(&src, &out, ctx).map(|_| ()),
                other => Err(Error::Invalid(format!("unknown conversion {other}"))),
            })
        }
        "ps4_search" => {
            let query = arg_required(args, "query")?;
            let source = arg_str(args, "source").unwrap_or_else(|| "gamesearch".into());
            if source == "ia" {
                Ok(to_text(&sources::ia_search(&query, 30)?))
            } else {
                Ok(to_text(&sources::gamesearch(
                    &config.gamesearch_base,
                    &query,
                    40,
                )?))
            }
        }
        "ps4_homebrew" => {
            let refresh = args
                .get("refresh")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(to_text(&homebrew::catalog(refresh)))
        }
        "ps4_homebrew_install" => {
            let id = arg_required(args, "id")?;
            let host = config.require_active()?;
            let entry = homebrew::catalog(false)
                .into_iter()
                .find(|e| e.id == id)
                .ok_or_else(|| Error::NotFound(format!("no homebrew {id}")))?;
            run_job("homebrew", move |ctx| {
                let report = homebrew::install(&host, &config, &entry, ctx)?;
                ctx.message(report.message);
                Ok(())
            })
        }
        "ps4_saves" => Ok(to_text(&saves::list(&config.require_active()?)?)),
        "ps4_save_backup" => {
            let wanted: Vec<String> = args
                .get("title_ids")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let out = PathBuf::from(arg_required(args, "out")?);
            let host = config.require_active()?;
            run_job("save backup", move |ctx| {
                let all = saves::list(&host)?;
                let selected: Vec<_> = all
                    .into_iter()
                    .filter(|s| wanted.is_empty() || wanted.iter().any(|id| id == &s.title_id))
                    .collect();
                let bytes = saves::backup(&host, &selected, &out, ctx)?;
                ctx.message(format!("wrote {bytes} bytes to {}", out.display()));
                Ok(())
            })
        }
        "ps4_pad_status" => Ok(to_text(&pad::status(&config.require_active()?))),
        other => Err(Error::Invalid(format!("unknown tool {other}"))),
    }
}

fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn arg_required(args: &Value, key: &str) -> Result<String> {
    arg_str(args, key).ok_or_else(|| Error::Invalid(format!("missing argument '{key}'")))
}

fn to_text(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into())
}

/// Run a core job and block until it ends. The MCP client sees one tool
/// result; there is no progress stream to attach to.
fn run_job<F>(label: &str, work: F) -> Result<String>
where
    F: FnOnce(&JobContext) -> Result<()> + Send + 'static,
{
    let manager = JobManager::new();
    let id = manager.spawn("mcp", label, |_| {}, work);
    let deadline = Instant::now() + Duration::from_secs(1800);
    loop {
        if Instant::now() > deadline {
            manager.cancel(id);
            return Err(Error::Other(format!(
                "{label} did not finish in 30 minutes"
            )));
        }
        std::thread::sleep(Duration::from_millis(150));
        let Some(job) = manager.get(id) else { continue };
        match job.state {
            godsend_core::JobState::Running => {}
            godsend_core::JobState::Done => return Ok(format!("{label}: {}", job.message)),
            godsend_core::JobState::Failed => return Err(Error::Other(job.message)),
            godsend_core::JobState::Cancelled => {
                return Err(Error::Other(format!("{label} cancelled")))
            }
        }
    }
}

/// stdio transport: one JSON message per line, one response per request.
pub fn run_stdio() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_message(&message),
            Err(err) => Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": format!("parse error: {err}") }
            })),
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Loopback HTTP transport (JSON responses, no SSE). Started by the desktop
/// app so an MCP client can connect while the app is open.
pub fn serve_http(port: u16) -> std::io::Result<()> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
    let port = listener.local_addr()?.port();
    tracing::info!("MCP server listening on http://127.0.0.1:{port}/mcp");
    serve_http_listener(listener)
}

/// Serve on an already-bound listener, so tests can pick an ephemeral port.
pub fn serve_http_listener(listener: std::net::TcpListener) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        std::thread::spawn(move || {
            let _ = handle_http(stream);
        });
    }
    Ok(())
}

fn handle_http(mut stream: std::net::TcpStream) -> std::io::Result<()> {
    use std::io::BufReader;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if request_line.trim().is_empty() {
        return Ok(());
    }
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let method = request_line.split_whitespace().next().unwrap_or("");
    if method == "OPTIONS" {
        return write_http(&mut stream, 204, "");
    }
    if method != "POST" {
        return write_http(&mut stream, 405, "use POST /mcp");
    }
    if content_length == 0 || content_length > 4 * 1024 * 1024 {
        return write_http(&mut stream, 400, "empty or oversized body");
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    let message: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => {
            return write_http(
                &mut stream,
                200,
                &json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                })
                .to_string(),
            )
        }
    };
    match handle_message(&message) {
        Some(response) => write_http(&mut stream, 200, &response.to_string()),
        None => write_http(&mut stream, 202, ""),
    }
}

fn write_http(stream: &mut std::net::TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type, mcp-session-id\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

/// One-line status for the app's Doctor page.
pub fn endpoint(port: u16) -> String {
    if port == 0 {
        "disabled".into()
    } else {
        format!("http://127.0.0.1:{port}/mcp")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_negotiates_and_names_the_server() {
        let response = handle_message(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }
        }))
        .expect("response");
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_has_schemas_and_unique_names() {
        let response =
            handle_message(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 15);
        let mut names = Vec::new();
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            assert!(name.starts_with("ps4_"));
            assert!(tool["description"].as_str().unwrap().len() > 10);
            assert!(tool["inputSchema"].is_object());
            names.push(name.to_string());
        }
        names.sort();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn notifications_get_no_response() {
        assert!(handle_message(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        )
        .is_none());
    }

    #[test]
    fn unknown_method_is_an_error() {
        let response =
            handle_message(&json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" })).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn tool_errors_are_results_not_protocol_errors() {
        let response = handle_message(&json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "ps4_inspect", "arguments": { "path": "/nonexistent/game.pkg" } }
        }))
        .unwrap();
        assert!(response["result"]["isError"].as_bool().unwrap());
    }

    #[test]
    fn endpoint_text() {
        assert_eq!(endpoint(0), "disabled");
        assert!(endpoint(8744).ends_with("/mcp"));
    }
}
