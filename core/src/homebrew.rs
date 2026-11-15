//! PS4 homebrew catalog: GitHub projects that publish a ready-to-install PKG
//! or a payload.
//!
//! Each entry names a repository and the asset filename fragment to pick from
//! its latest release, so a refresh is one API call per project and never
//! depends on release ordering. A `pkg` entry goes through the console's
//! installer; a `payload` entry goes through the bin loader.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::install::{install_local, InstallReport};
use crate::transfer::JobContext;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// `pad` | `save` | `tool` | `payload`
    pub category: String,
    pub repo: String,
    /// Asset filename fragment, e.g. `APOLLO`.
    pub asset: String,
    /// `pkg` installs through the console; `payload` through the bin loader.
    pub kind: String,
    pub note: Option<String>,
    #[serde(default)]
    pub latest_tag: Option<String>,
    #[serde(default)]
    pub asset_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
}

pub fn bundled_catalog() -> Vec<CatalogEntry> {
    let entry = |id: &str,
                 name: &str,
                 description: &str,
                 category: &str,
                 repo: &str,
                 asset: &str,
                 kind: &str,
                 note: Option<&str>| CatalogEntry {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: category.into(),
        repo: repo.into(),
        asset: asset.into(),
        kind: kind.into(),
        note: note.map(|n| n.to_string()),
        latest_tag: None,
        asset_url: None,
        published_at: None,
        size_bytes: 0,
    };
    vec![
        entry(
            "control4free",
            "Control4Free",
            "Virtual DualShock 4 over the network, up to four controllers. The Controller page talks to it on port 4264.",
            "pad",
            "MoHadiShibli/Control4Free",
            "Control4Free-",
            "pkg",
            Some("Install this to use the Controller page. It also runs on the console itself from its own menu."),
        ),
        entry(
            "apollo",
            "Apollo Save Tool",
            "Save data manager: back up, restore and resign saves, and download saves from the community database.",
            "save",
            "bucanero/apollo-ps4",
            "APOLLO",
            "pkg",
            None,
        ),
    ]
}

/// Refreshed catalog rows for the UI, cached on disk between refreshes.
pub fn catalog(refresh: bool) -> Vec<CatalogEntry> {
    let mut entries = bundled_catalog();
    let cache = cache_path();
    if !refresh {
        if let Ok(bytes) = std::fs::read(&cache) {
            if let Ok(cached) = serde_json::from_slice::<Vec<CatalogEntry>>(&bytes) {
                if cached.len() == entries.len() {
                    return cached;
                }
            }
        }
    }
    for entry in &mut entries {
        if let Ok(release) = latest_release(&entry.repo) {
            if let Some((tag, url, published, size)) = pick_asset(&release, &entry.asset, &entry.kind)
            {
                entry.latest_tag = Some(tag);
                entry.asset_url = Some(url);
                entry.published_at = Some(published);
                entry.size_bytes = size;
            }
        }
    }
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&entries) {
        let _ = std::fs::write(&cache, bytes);
    }
    entries
}

fn cache_path() -> PathBuf {
    crate::config::cache_dir().join("homebrew.json")
}

fn latest_release(repo: &str) -> Result<serde_json::Value> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(20))
        .build();
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    agent
        .get(&url)
        .set("User-Agent", "godsend-ps4")
        .set("Accept", "application/vnd.github+json")
        .call()?
        .into_json()
        .map_err(|e| Error::Http(format!("github {repo}: {e}")))
}

/// Choose the release asset matching `fragment` and the entry's kind.
fn pick_asset(
    release: &serde_json::Value,
    fragment: &str,
    kind: &str,
) -> Option<(String, String, String, u64)> {
    let tag = release.get("tag_name")?.as_str()?.to_string();
    let published = release
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let assets = release.get("assets")?.as_array()?;
    let asset = assets.iter().find(|a| {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase();
        if name.contains("troubleshooting") || name.contains("diag") {
            return false;
        }
        name.contains(&fragment.to_lowercase())
            && (if kind == "payload" {
                name.ends_with(".elf") || name.ends_with(".bin")
            } else {
                name.ends_with(".pkg")
            })
    })?;
    Some((
        tag,
        asset.get("browser_download_url")?.as_str()?.to_string(),
        published,
        asset.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
    ))
}

/// Download a catalog entry's file, then install it on the console.
pub fn install(
    host: &crate::config::Host,
    config: &Config,
    entry: &CatalogEntry,
    ctx: &JobContext,
) -> Result<InstallReport> {
    let url = entry.asset_url.as_ref().ok_or_else(|| {
        Error::NotFound(format!(
            "{} has no matching release asset; refresh the catalog",
            entry.name
        ))
    })?;
    let name = url.rsplit('/').next().unwrap_or("homebrew.pkg").to_string();
    ctx.message(format!("downloading {}", entry.name));
    let local = crate::store::download_url(url, &name, config, ctx)?;
    ctx.message(format!("installing {}", entry.name));
    install_local(host, &local, config, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_unique() {
        let entries = bundled_catalog();
        assert!(entries.len() >= 2);
        let mut ids: Vec<_> = entries.iter().map(|e| e.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), entries.len());
        assert!(entries.iter().all(|e| e.repo.contains('/')));
        assert!(entries.iter().any(|e| e.id == "control4free"));
    }

    #[test]
    fn picks_the_matching_asset() {
        let release = serde_json::json!({
            "tag_name": "1.2.3",
            "published_at": "2026-10-10T20:18:40Z",
            "assets": [
                {"name": "Control4Free-1.3.1.pkg", "browser_download_url": "https://x/c.pkg", "size": 7012352},
                {"name": "troubleshooting-Control4Free-1.3.1-diag.pkg", "browser_download_url": "https://x/d.pkg", "size": 1},
                {"name": "control4free.elf", "browser_download_url": "https://x/c.elf", "size": 1839600}
            ]
        });
        let (_, url, _, size) = pick_asset(&release, "Control4Free-", "pkg").expect("asset");
        assert!(url.ends_with("c.pkg"));
        assert_eq!(size, 7012352);
        let (_, elf, _, _) = pick_asset(&release, "control4free", "payload").expect("payload");
        assert!(elf.ends_with("c.elf"));
        assert!(pick_asset(&release, "missing", "pkg").is_none());
    }
}
