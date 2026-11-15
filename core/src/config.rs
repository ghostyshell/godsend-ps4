//! App configuration: PS4 hosts, download folders, debrid keys and service
//! bases. Persisted as JSON under the platform data directory.

use crate::error::{Error, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// One PS4 running GoldHEN (or any payload with FTP, RPI and the remote pad).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Host {
    /// Stable id: `addr:ftp_port`.
    pub id: String,
    pub name: String,
    /// IP address or hostname.
    pub addr: String,
    #[serde(default = "default_ftp_port")]
    pub ftp_port: u16,
    #[serde(default = "default_ftp_user")]
    pub ftp_user: String,
    #[serde(default)]
    pub ftp_pass: String,
    /// GoldHEN remote package install port.
    #[serde(default = "default_rpi_port")]
    pub rpi_port: u16,
    /// Remote pad service port (Control4Free).
    #[serde(default = "default_pad_port")]
    pub pad_port: u16,
    /// GoldHEN payload loader port.
    #[serde(default = "default_binloader_port")]
    pub binloader_port: u16,
}

fn default_ftp_port() -> u16 {
    2121
}
fn default_ftp_user() -> String {
    "anonymous".into()
}
fn default_rpi_port() -> u16 {
    12800
}
fn default_pad_port() -> u16 {
    4264
}
fn default_binloader_port() -> u16 {
    9090
}

impl Host {
    pub fn new(name: &str, addr: &str) -> Self {
        let addr = addr.trim().to_string();
        Self {
            id: format!("{addr}:{}", default_ftp_port()),
            name: if name.trim().is_empty() {
                addr.clone()
            } else {
                name.trim().to_string()
            },
            addr,
            ftp_port: default_ftp_port(),
            ftp_user: default_ftp_user(),
            ftp_pass: String::new(),
            rpi_port: default_rpi_port(),
            pad_port: default_pad_port(),
            binloader_port: default_binloader_port(),
        }
    }

    /// Recompute the id after the address or port changed.
    pub fn refresh_id(&mut self) {
        self.id = format!("{}:{}", self.addr.trim(), self.ftp_port);
    }

    pub fn rpi_base(&self) -> String {
        format!("http://{}:{}", self.addr, self.rpi_port)
    }

    pub fn pad_base(&self) -> String {
        format!("http://{}:{}", self.addr, self.pad_port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Every configured console.
    pub hosts: Vec<Host>,
    /// Id of the host the UI works against.
    pub active_host: Option<String>,
    /// Where downloads land. Empty = `<Downloads>/godsend-ps4`.
    pub download_dir: String,
    /// Real-Debrid API key. Empty = not connected.
    pub rd_api_key: String,
    /// TorBox API key. Empty = not connected.
    pub tb_api_key: String,
    /// game-search service base URL (the debrid.link-style index).
    pub gamesearch_base: String,
    /// Public R2 base for box art.
    pub covers_base: String,
    /// Optional explicit aria2c path. Empty = auto-detect.
    pub aria2_path: String,
    /// Chunk size for FAT32 splits in bytes (default 4 GiB - 1 MiB).
    pub split_bytes: u64,
    /// Hand a package to the console's remote installer after the download.
    pub auto_install_pkg: bool,
    /// Loopback port for the bundled MCP server. 0 disables it.
    pub mcp_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hosts: Vec::new(),
            active_host: None,
            download_dir: String::new(),
            rd_api_key: String::new(),
            tb_api_key: String::new(),
            gamesearch_base: "http://localhost:3011".into(),
            covers_base: "https://covers.debrid.in".into(),
            aria2_path: String::new(),
            split_bytes: 4 * 1024 * 1024 * 1024 - 1024 * 1024,
            auto_install_pkg: true,
            mcp_port: 8744,
        }
    }
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!(
                    "config at {} is corrupt ({}), using defaults",
                    path.display(),
                    e
                );
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn active(&self) -> Option<Host> {
        let id = self.active_host.as_ref()?;
        self.hosts.iter().find(|h| &h.id == id).cloned()
    }

    pub fn require_active(&self) -> Result<Host> {
        self.active().ok_or(Error::NoDevice)
    }

    /// Add or replace a host by id, then make it active when it is the first.
    pub fn upsert_host(&mut self, mut host: Host) {
        host.refresh_id();
        host.name = host.name.trim().to_string();
        host.addr = host.addr.trim().to_string();
        if host.name.is_empty() {
            host.name = host.addr.clone();
        }
        if let Some(existing) = self.hosts.iter_mut().find(|h| h.id == host.id) {
            *existing = host.clone();
        } else {
            self.hosts.push(host.clone());
        }
        if self.active_host.is_none() {
            self.active_host = Some(host.id);
        }
    }

    pub fn remove_host(&mut self, id: &str) {
        self.hosts.retain(|h| h.id != id);
        if self.active_host.as_deref() == Some(id) {
            self.active_host = self.hosts.first().map(|h| h.id.clone());
        }
    }

    /// Overwrite only the fields present in `patch` (a partial JSON object).
    pub fn patch_from_json(&mut self, patch: &serde_json::Value) -> Result<()> {
        let merged = merge_json(&serde_json::to_value(&*self)?, patch);
        let next: Config = serde_json::from_value(merged)
            .map_err(|e| Error::Invalid(format!("config patch rejected: {}", e)))?;
        *self = next;
        self.save()
    }
}

fn merge_json(base: &serde_json::Value, patch: &serde_json::Value) -> serde_json::Value {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            let mut out = b.clone();
            for (k, v) in p {
                let merged = match out.get(k) {
                    Some(existing) => merge_json(existing, v),
                    None => v.clone(),
                };
                out.insert(k.clone(), merged);
            }
            serde_json::Value::Object(out)
        }
        (_, v) => v.clone(),
    }
}

/// `click.godsendps4.app` style qualifier so the data dir is
/// `~/Library/Application Support/godsend-ps4` on macOS and friends elsewhere.
pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("click", "godsendps4", "godsend-ps4")
}

pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("godsend-ps4"))
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn icons_cache_dir() -> PathBuf {
    cache_dir().join("icons")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn jobs_path() -> PathBuf {
    data_dir().join("jobs.json")
}

pub fn ensure_dirs() -> Result<()> {
    for dir in [data_dir(), cache_dir(), icons_cache_dir()] {
        fs::create_dir_all(&dir)?;
        probe_writable(&dir)?;
    }
    Ok(())
}

fn probe_writable(dir: &Path) -> Result<()> {
    let probe = dir.join(".write-probe");
    fs::write(&probe, b"ok")?;
    fs::remove_file(&probe)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_ids_and_active_selection() {
        let mut cfg = Config::default();
        let mut host = Host::new("Living room", "192.168.1.50");
        cfg.upsert_host(host.clone());
        assert_eq!(cfg.active_host.as_deref(), Some("192.168.1.50:2121"));
        assert_eq!(host.ftp_port, 2121);
        assert_eq!(host.rpi_port, 12800);
        host.ftp_port = 21;
        host.addr = "192.168.1.51".into();
        cfg.upsert_host(host);
        assert_eq!(cfg.hosts.len(), 2);
        assert_eq!(cfg.require_active().unwrap().addr, "192.168.1.50");
        cfg.remove_host("192.168.1.50:2121");
        assert_eq!(cfg.require_active().unwrap().addr, "192.168.1.51");
    }

    #[test]
    fn patch_merges_fields() {
        let mut cfg = Config::default();
        cfg.patch_from_json(&serde_json::json!({ "auto_install_pkg": false, "split_bytes": 123 }))
            .expect("patch must apply");
        assert!(!cfg.auto_install_pkg);
        assert_eq!(cfg.split_bytes, 123);
    }
}
