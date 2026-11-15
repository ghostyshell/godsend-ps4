//! Remote library scan: what is installed on the console and what is waiting
//! to be installed.
//!
//! GoldHEN exposes the console filesystem over FTP:
//!
//! - `/user/appmeta/<TITLE_ID>/param.json` for every installed title
//!   (games, patches, DLC, homebrew), with `icon0.png` beside it
//! - `/user/appmeta/external/<TITLE_ID>` for titles installed to USB storage
//! - `/user/pkg/*.pkg` for packages uploaded and not yet installed
//! - `/user/homebrew` for loose homebrew files, when it exists
//!
//! One unreadable `param.json` never empties the library: every entry is read
//! independently and falls back to its folder name.

use crate::config::Host;
use crate::error::{Error, Result};
use crate::ftp;
use crate::param::{self, TitleInfo};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct GameEntry {
    /// Stable id: `<kind>:<remote path>`.
    pub id: String,
    pub title: String,
    pub title_id: Option<String>,
    pub content_id: Option<String>,
    /// Always `ps4` for now; kept so the UI can group later.
    pub system: String,
    /// `app` | `homebrew` | `pkg`.
    pub kind: String,
    /// `gd` | `gp` | `ac` | `homebrew` | `package`.
    pub category: String,
    pub path: String,
    pub size_bytes: u64,
    pub has_icon: bool,
    pub icon_path: Option<String>,
    pub version: Option<String>,
    /// `internal` | `external` | `usb0` | ...
    pub device: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LibraryView {
    /// Installed titles.
    pub games: Vec<GameEntry>,
    /// Packages waiting in `/user/pkg` or on a USB volume.
    pub packages: Vec<GameEntry>,
    /// Directories that were reachable, for the empty state.
    pub roots: Vec<String>,
}

/// Volumes GoldHEN exposes under `/mnt`.
const MNT_VOLUMES: &[&str] = &["/mnt/usb0", "/mnt/usb1", "/mnt/ext0", "/mnt/ext1"];

pub fn scan(host: &Host) -> Result<LibraryView> {
    let mut conn = ftp::connect(host)?;
    let mut view = LibraryView::default();

    scan_appmeta(&mut conn, &mut view, "/user/appmeta", "internal");
    scan_appmeta(&mut conn, &mut view, "/user/appmeta/external", "external");

    if let Ok(entries) = ftp::list(&mut conn, "/user/pkg") {
        view.roots.push("/user/pkg".into());
        push_packages(&mut conn, &mut view, entries, "internal");
    }

    for volume in MNT_VOLUMES {
        let Ok(entries) = ftp::list(&mut conn, volume) else {
            continue;
        };
        view.roots.push((*volume).into());
        let device = volume.trim_start_matches("/mnt/").to_string();
        push_packages(&mut conn, &mut view, entries.clone(), &device);
        // A USB game folder installed with a PKG carries its own appmeta.
        scan_appmeta(
            &mut conn,
            &mut view,
            &format!("{volume}/user/appmeta"),
            &device,
        );
    }

    if let Ok(entries) = ftp::list(&mut conn, "/user/homebrew") {
        view.roots.push("/user/homebrew".into());
        for entry in entries {
            let lower = entry.name.to_lowercase();
            if entry.is_dir || !(lower.ends_with(".elf") || lower.ends_with(".pkg")) {
                continue;
            }
            view.games.push(GameEntry {
                id: format!("homebrew:{}", entry.path),
                title: title_from_filename(&entry.name),
                title_id: None,
                content_id: None,
                system: "ps4".into(),
                kind: "homebrew".into(),
                category: "homebrew".into(),
                path: entry.path,
                size_bytes: entry.size_bytes,
                has_icon: false,
                icon_path: None,
                version: None,
                device: "internal".into(),
            });
        }
    }

    let _ = conn.quit();
    view.games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    view.packages
        .sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    view.games.dedup_by(|a, b| a.path == b.path);
    Ok(view)
}

fn scan_appmeta(
    conn: &mut suppaftp::FtpStream,
    view: &mut LibraryView,
    dir: &str,
    device: &str,
) {
    let Ok(entries) = ftp::list(conn, dir) else {
        return;
    };
    if !entries.is_empty() {
        view.roots.push(dir.to_string());
    }
    for entry in entries.iter().filter(|e| e.is_dir) {
        // `/user/appmeta/external` is the USB meta tree, scanned on its own.
        if entry.name == "external" {
            continue;
        }
        let meta = read_meta_dir(conn, &entry.path).unwrap_or_default();
        // Patches and DLC belong to a game, not to the library grid.
        if meta.category == "gp" || meta.category == "ac" {
            continue;
        }
        let title = meta
            .title
            .clone()
            .unwrap_or_else(|| entry.name.clone());
        // Icons are fetched lazily by the grid; a missing one shows a monogram.
        let icon = Some(format!("{}/icon0.png", entry.path));
        view.games.push(GameEntry {
            id: format!("app:{}", entry.path),
            title,
            title_id: meta.title_id.clone().or_else(|| Some(entry.name.clone())),
            content_id: meta.content_id.clone(),
            system: "ps4".into(),
            kind: if meta.category == "homebrew" {
                "homebrew".into()
            } else {
                "app".into()
            },
            category: meta.category.clone(),
            path: entry.path.clone(),
            size_bytes: 0,
            has_icon: icon.is_some(),
            icon_path: icon,
            version: meta.version.clone(),
            device: device.into(),
        });
    }
}

fn push_packages(
    conn: &mut suppaftp::FtpStream,
    view: &mut LibraryView,
    entries: Vec<ftp::RemoteEntry>,
    device: &str,
) {
    for entry in entries.into_iter().filter(|e| !e.is_dir) {
        if !entry.name.to_lowercase().ends_with(".pkg") {
            continue;
        }
        let head = ftp::read_head(conn, &entry.path, 4096).unwrap_or_default();
        let info = crate::pkg::from_head(&head);
        view.packages.push(GameEntry {
            id: format!("pkg:{}", entry.path),
            title: title_from_filename(&entry.name),
            title_id: info.title_id,
            content_id: info.content_id,
            system: "ps4".into(),
            kind: "pkg".into(),
            category: "package".into(),
            path: entry.path,
            size_bytes: entry.size_bytes,
            has_icon: false,
            icon_path: None,
            version: None,
            device: device.into(),
        });
    }
}

/// `God of War (USA).pkg` -> `God of War`.
pub fn title_from_filename(name: &str) -> String {
    let stem = name.rsplit('/').next().unwrap_or(name);
    let stem = stem
        .trim_end_matches(".pkg")
        .trim_end_matches(".PKG")
        .trim_end_matches(".elf")
        .trim_end_matches(".ELF");
    let before_tag = stem.split(" (").next().unwrap_or(stem);
    let cleaned = before_tag.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        stem.to_string()
    } else {
        cleaned
    }
}

/// Download and cache an icon for a library entry.
pub fn icon_bytes(host: &Host, icon_path: &str) -> Option<Vec<u8>> {
    let cache = icon_cache_path(icon_path);
    if let Ok(bytes) = std::fs::read(&cache) {
        return Some(bytes);
    }
    let mut conn = ftp::connect(host).ok()?;
    let bytes = ftp::read_head(&mut conn, icon_path, 4 * 1024 * 1024).ok()?;
    let _ = conn.quit();
    if bytes.is_empty() {
        return None;
    }
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&cache, &bytes);
    Some(bytes)
}

fn icon_cache_path(icon_path: &str) -> PathBuf {
    crate::config::icons_cache_dir().join(format!("{:016x}.png", fnv1a(icon_path)))
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Delete a package or a loose homebrew file. Installed titles are managed by
/// the console's own database, so deleting their folders over FTP would leave
/// a ghost entry; the app refuses instead.
pub fn delete_entry(host: &Host, entry_path: &str, is_dir: bool) -> Result<()> {
    if entry_path.starts_with("/user/app/") || entry_path.starts_with("/user/appmeta/") {
        return Err(Error::Invalid(
            "installed titles are removed from the PS4's own storage menu, not over FTP".into(),
        ));
    }
    let mut conn = ftp::connect(host)?;
    let result = ftp::remove(&mut conn, entry_path, is_dir);
    let _ = conn.quit();
    result
}

/// Read a title's metadata from an already-connected session.
pub fn read_meta(conn: &mut suppaftp::FtpStream, title_id: &str) -> Option<TitleInfo> {
    read_meta_dir(conn, &format!("/user/appmeta/{title_id}"))
}

/// Read `param.json` from any appmeta folder (internal or external).
pub fn read_meta_dir(conn: &mut suppaftp::FtpStream, dir: &str) -> Option<TitleInfo> {
    let body = ftp::read_head(conn, &format!("{dir}/param.json"), 128 * 1024).ok()?;
    let text = String::from_utf8_lossy(&body).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(param::parse(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_from_filenames() {
        assert_eq!(title_from_filename("God of War (USA).pkg"), "God of War");
        assert_eq!(
            title_from_filename("/user/pkg/Bloodborne.pkg"),
            "Bloodborne"
        );
        assert_eq!(title_from_filename("homebrew.elf"), "homebrew");
    }

    #[test]
    fn hashes_are_stable() {
        assert_eq!(fnv1a("abc"), fnv1a("abc"));
        assert_ne!(fnv1a("abc"), fnv1a("abd"));
    }
}
