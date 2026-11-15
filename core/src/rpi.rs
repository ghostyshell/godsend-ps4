//! Remote package install: hand a local `.pkg` to GoldHEN and let the console
//! pull it over HTTP.
//!
//! GoldHEN's installer (the API from `flatz/ps4_remote_pkg_installer`) listens
//! on port 12800 and takes a URL, not an upload. So the app serves the file on
//! a one-shot HTTP listener bound to the interface that reaches the console,
//! then posts:
//!
//! ```json
//! {"type": "direct", "packages": ["http://192.168.1.10:52344/game.pkg"]}
//! ```
//!
//! The console fetches it in the background and installs it.

use crate::config::Host;
use crate::error::{Error, Result};
use crate::transfer::JobContext;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(90);

/// True when the installer answers on `/api/status`.
pub fn available(host: &Host) -> bool {
    let url = format!("{}/api/status", host.rpi_base());
    match ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .build()
        .get(&url)
        .call()
    {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

/// Queue `local` on the console and serve it over HTTP until the transfer ends.
pub fn install(host: &Host, local: &Path, ctx: &JobContext) -> Result<u64> {
    let size = std::fs::metadata(local)?.len();
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("package has no file name".into()))?;
    let lan = lan_ip(host).ok_or_else(|| {
        Error::Rpi("could not work out the LAN address the console should reach".into())
    })?;

    let listener = TcpListener::bind(("0.0.0.0", 0))
        .map_err(|e| Error::Rpi(format!("cannot open a serving socket: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::Rpi(e.to_string()))?
        .port();
    let url = format!("http://{lan}:{port}/{}", url_encode(&name));

    ctx.message(format!("asking the console to install {name}"));
    let body = json!({ "type": "direct", "packages": [url] });
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .post(&format!("{}/api/install", host.rpi_base()))
        .send_json(body);
    match response {
        Ok(resp) => {
            let text = resp.into_string().unwrap_or_default();
            if !text.is_empty() {
                ctx.message(format!("console: {}", text.trim()));
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            return Err(Error::Rpi(format!(
                "the installer refused the package (HTTP {code}) {text}"
            )));
        }
        Err(e) => return Err(Error::Rpi(format!("install request failed: {e}"))),
    }

    // The console opens the transfer within a second or two. Wait for it, but
    // stay cancellable, so the listener must not block.
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::Rpi(e.to_string()))?;
    let started = Instant::now();
    let accepted = loop {
        ctx.check_cancel()?;
        if started.elapsed() > ACCEPT_TIMEOUT {
            return Err(Error::Rpi(
                "the console never started the download; check that the installer has focus".into(),
            ));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                break stream;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(Error::Rpi(e.to_string())),
        }
    };
    let served = serve(accepted, local, size, ctx)?;
    ctx.message("package delivered; the console installs it in the background");
    Ok(served)
}

/// Answer one GET (including a resume range) and stream the file.
fn serve(mut stream: TcpStream, local: &Path, size: u64, ctx: &JobContext) -> Result<u64> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        request.push_str(&line);
    }
    let mut start = 0u64;
    for line in request.lines() {
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
            let first = value.split('-').next().unwrap_or("").trim();
            if let Ok(offset) = first.parse::<u64>() {
                start = offset.min(size);
            }
        }
    }
    let length = size.saturating_sub(start);
    if start > 0 {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {length}\r\nContent-Range: bytes {start}-{}/{size}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                size.saturating_sub(1)
            )
            .as_bytes(),
        );
    } else {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        );
    }
    let mut file = std::fs::File::open(local)?;
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = start;
    ctx.progress(done, size);
    loop {
        ctx.check_cancel()?;
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        if stream.write_all(&buf[..read]).is_err() {
            // The console closed early; the install will retry or fail there.
            break;
        }
        done += read as u64;
        ctx.progress(done, size);
    }
    let _ = stream.flush();
    Ok(done.saturating_sub(start))
}

/// The local address that reaches this console.
fn lan_ip(host: &Host) -> Option<String> {
    let addr: SocketAddr = format!("{}:{}", host.addr, host.rpi_port).parse().ok()?;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(addr).ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// Percent-encode the characters a file name can legally contain but a URL
/// cannot.
fn url_encode(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.bytes() {
        let safe = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_names_with_spaces() {
        assert_eq!(url_encode("God of War.pkg"), "God%20of%20War.pkg");
        assert_eq!(url_encode("a_b-c.d~e"), "a_b-c.d~e");
    }

    #[test]
    fn refuses_a_missing_file() {
        let host = Host::new("test", "127.0.0.1");
        let manager = crate::transfer::JobManager::new();
        let (tx, rx) = std::sync::mpsc::channel();
        manager.spawn(
            "install",
            "missing",
            |_| {},
            move |ctx| {
                let result = install(&host, Path::new("/nonexistent/game.pkg"), ctx);
                tx.send(result.is_err()).unwrap();
                Ok(())
            },
        );
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
    }
}
