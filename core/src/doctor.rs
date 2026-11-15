//! Preflight checks shown on the Home page.

use crate::config::{Config, Host};
use crate::device;
use crate::error::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub fn run(config: &Config) -> Vec<Check> {
    let mut checks = Vec::new();

    match config.active() {
        Some(host) => checks.push(Check {
            name: "Console configured".into(),
            ok: true,
            detail: format!("{} at {}", host.name, host.addr),
        }),
        None => checks.push(Check {
            name: "Console configured".into(),
            ok: false,
            detail: "Add a PS4 on the Devices page".into(),
        }),
    }

    if let Some(host) = config.active() {
        let status = device::probe(&host);
        checks.push(Check {
            name: "FTP reachable".into(),
            ok: status.ftp_ok,
            detail: if status.ftp_ok {
                format!(
                    "{} ms{}",
                    status.ftp_ms,
                    status
                        .greeting
                        .as_deref()
                        .map(|g| format!(" - {g}"))
                        .unwrap_or_default()
                )
            } else {
                "start GoldHEN on the console and enable its FTP service (port 2121)".into()
            },
        });
        checks.push(Check {
            name: "Package installer".into(),
            ok: status.rpi_ok,
            detail: if status.rpi_ok {
                format!("port {} answers", host.rpi_port)
            } else {
                format!(
                    "port {} did not answer; packages will be uploaded to /user/pkg instead",
                    host.rpi_port
                )
            },
        });
        checks.push(Check {
            name: "Virtual pad".into(),
            ok: status.pad_ok,
            detail: if status.pad_ok {
                format!("port {} answers", host.pad_port)
            } else {
                "install Control4Free from the Homebrew page to use the Controller".into()
            },
        });
        if status.titles > 0 || status.packages > 0 {
            checks.push(Check {
                name: "Library".into(),
                ok: true,
                detail: format!(
                    "{} titles installed, {} packages waiting",
                    status.titles, status.packages
                ),
            });
        }
    }

    let download_dir = crate::aria2::download_dir(config);
    let writable = std::fs::create_dir_all(&download_dir).is_ok()
        && std::fs::write(download_dir.join(".probe"), b"ok").is_ok();
    if writable {
        let _ = std::fs::remove_file(download_dir.join(".probe"));
    }
    checks.push(Check {
        name: "Download folder".into(),
        ok: writable,
        detail: download_dir.display().to_string(),
    });

    checks.push(Check {
        name: "aria2c".into(),
        ok: crate::aria2::find_aria2(&config.aria2_path).is_some(),
        detail: match crate::aria2::find_aria2(&config.aria2_path) {
            Some(path) => path.display().to_string(),
            None => "optional: downloads fall back to a single connection".into(),
        },
    });

    checks.push(Check {
        name: "Debrid".into(),
        ok: !config.rd_api_key.is_empty() || !config.tb_api_key.is_empty(),
        detail: match (config.rd_api_key.is_empty(), config.tb_api_key.is_empty()) {
            (false, false) => "Real-Debrid and TorBox connected".into(),
            (false, true) => "Real-Debrid connected".into(),
            (true, false) => "TorBox connected".into(),
            (true, true) => "optional: keys unlock debrid download links".into(),
        },
    });

    checks.push(Check {
        name: "MCP server".into(),
        ok: config.mcp_port > 0,
        detail: if config.mcp_port == 0 {
            "disabled (set an MCP port to enable it)".into()
        } else {
            format!(
                "http://127.0.0.1:{}/mcp, started with the app",
                config.mcp_port
            )
        },
    });

    checks
}

pub fn host_checks(host: &Host) -> Result<Vec<Check>> {
    let status = device::probe(host);
    Ok(vec![
        Check {
            name: "FTP".into(),
            ok: status.ftp_ok,
            detail: format!("{} ms", status.ftp_ms),
        },
        Check {
            name: "Package installer".into(),
            ok: status.rpi_ok,
            detail: format!("port {}", host.rpi_port),
        },
        Check {
            name: "Virtual pad".into(),
            ok: status.pad_ok,
            detail: format!("port {}", host.pad_port),
        },
    ])
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
    }
}
