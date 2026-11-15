//! PS4 host probing: FTP reachability, remote package install, virtual pad
//! service and a quick count of what is installed.
//!
//! A PS4 does not expose a system-info page the way webMAN does on the PS3,
//! so every field here is something the console actually answers over FTP or
//! HTTP. Nothing is guessed.

use crate::config::Host;
use crate::ftp;
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub const APP_META: &str = "/user/appmeta";
pub const PKG_DIR: &str = "/user/pkg";

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeviceStatus {
    pub host_id: String,
    pub name: String,
    pub addr: String,
    pub ftp_ok: bool,
    pub ftp_ms: u64,
    /// Server banner from the FTP control connection, e.g. "GoldHEN FTP server".
    pub greeting: Option<String>,
    /// GoldHEN remote package installer answers on `rpi_port`.
    pub rpi_ok: bool,
    pub rpi_ms: u64,
    /// Control4Free virtual pad service answers on `pad_port`.
    pub pad_ok: bool,
    pub pad_ms: u64,
    /// Titles in `/user/appmeta` (installed apps, homebrew, patches).
    pub titles: u32,
    /// Packages waiting in `/user/pkg`.
    pub packages: u32,
    pub message: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .build()
}

/// Probe a console. Never errors: unreachable pieces are reported as false.
pub fn probe(host: &Host) -> DeviceStatus {
    let mut status = DeviceStatus {
        host_id: host.id.clone(),
        name: host.name.clone(),
        addr: host.addr.clone(),
        ..Default::default()
    };

    // The FTP banner is served before login, so a bare socket is enough and it
    // names the payload for the device card.
    status.greeting = banner(host);

    let started = Instant::now();
    match ftp::connect(host) {
        Ok(mut conn) => {
            status.ftp_ok = conn.pwd().is_ok();
            status.ftp_ms = started.elapsed().as_millis() as u64;
            if let Ok(entries) = ftp::list(&mut conn, APP_META) {
                status.titles = entries.iter().filter(|e| e.is_dir).count() as u32;
            }
            if let Ok(entries) = ftp::list(&mut conn, PKG_DIR) {
                status.packages = entries
                    .iter()
                    .filter(|e| !e.is_dir && e.name.to_lowercase().ends_with(".pkg"))
                    .count() as u32;
            }
            let _ = conn.quit();
        }
        Err(e) => status.message = e.to_string(),
    }

    let started = Instant::now();
    if http_answers(&format!("{}/api/status", host.rpi_base())) {
        status.rpi_ok = true;
        status.rpi_ms = started.elapsed().as_millis() as u64;
    }

    let started = Instant::now();
    match agent().get(&format!("{}/api/status", host.pad_base())).call() {
        Ok(_) => {
            status.pad_ok = true;
            status.pad_ms = started.elapsed().as_millis() as u64;
        }
        Err(ureq::Error::Status(_, _)) => {
            // Any HTTP reply on the pad port means the payload is loaded.
            status.pad_ok = true;
            status.pad_ms = started.elapsed().as_millis() as u64;
        }
        Err(_) => {}
    }

    if status.message.is_empty() && !status.ftp_ok {
        status.message = "the FTP server did not answer; start GoldHEN and its FTP service".into();
    }
    status
}

/// Read the FTP 220 banner without logging in.
pub fn banner(host: &Host) -> Option<String> {
    let addr: SocketAddr = format!("{}:{}", host.addr, host.ftp_port).parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(1200)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
    let mut line = String::new();
    BufReader::new(stream.try_clone().ok()?)
        .read_line(&mut line)
        .ok()?;
    let _ = stream.write_all(b"QUIT\r\n");
    let text = line.trim().trim_start_matches("220 ").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// True when something answers HTTP on `url`, including 4xx.
fn http_answers(url: &str) -> bool {
    match agent().get(url).call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

/// Probe every configured host in parallel.
pub fn probe_all(hosts: &[Host]) -> Vec<DeviceStatus> {
    let mut handles = Vec::new();
    for host in hosts {
        let host = host.clone();
        handles.push(std::thread::spawn(move || probe(&host)));
    }
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Best-effort LAN discovery: a host answering FTP on 2121 or 21 is a candidate.
pub fn discover(subnet_prefix: &str) -> Vec<Host> {
    let mut found = Vec::new();
    let mut handles = Vec::new();
    for last in 1..=254u8 {
        let addr = format!("{subnet_prefix}.{last}");
        handles.push(std::thread::spawn(move || {
            for port in [2121u16, 21] {
                let sock: SocketAddr = format!("{addr}:{port}").parse().ok()?;
                if TcpStream::connect_timeout(&sock, Duration::from_millis(200)).is_ok() {
                    let mut host = Host::new("", &addr);
                    host.ftp_port = port;
                    host.refresh_id();
                    return Some(host);
                }
            }
            None
        }));
        if handles.len() >= 64 {
            for h in handles.drain(..) {
                if let Ok(Some(host)) = h.join() {
                    found.push(host);
                }
            }
        }
    }
    for h in handles {
        if let Ok(Some(host)) = h.join() {
            found.push(host);
        }
    }
    found.sort_by(|a, b| a.addr.cmp(&b.addr));
    let local = local_ipv4().unwrap_or_default();
    // A TCP connect only proves the port is open; confirm it speaks FTP.
    found
        .into_iter()
        .filter(|h| h.addr != local)
        .filter_map(|mut h| {
            let status = probe(&h);
            if !status.ftp_ok {
                return None;
            }
            h.name = status
                .greeting
                .clone()
                .unwrap_or_else(|| format!("PS4 {}", h.addr));
            Some(h)
        })
        .collect()
}

fn local_ipv4() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_the_banner_prefix() {
        let line = "220 GoldHEN FTP server ready\r\n";
        assert_eq!(
            line.trim().trim_start_matches("220 ").trim(),
            "GoldHEN FTP server ready"
        );
    }

    #[test]
    fn an_absent_host_is_offline() {
        let mut host = Host::new("ghost", "127.0.0.1");
        host.ftp_port = 1; // nothing listens on tcpmux
        host.rpi_port = 1;
        host.pad_port = 1;
        let status = probe(&host);
        assert!(!status.ftp_ok);
        assert!(!status.rpi_ok);
        assert!(!status.pad_ok);
    }
}
