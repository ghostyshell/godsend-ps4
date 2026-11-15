//! Save data: list, back up and restore what lives under
//! `/user/home/<user>/savedata/<TITLE_ID>`.
//!
//! A backup is a zip of the save folder, so it can be unzipped into the same
//! place by hand if the app is gone. The zip keeps the title id as its top
//! folder, which is what a restore needs to know where to put the files.

use crate::config::Host;
use crate::error::{Error, Result};
use crate::ftp;
use crate::transfer::JobContext;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SaveEntry {
    pub title_id: String,
    /// Game title from appmeta, when the console still has the game installed.
    pub title: Option<String>,
    /// `/user/home/<user>/savedata/<TITLE_ID>`
    pub path: String,
    pub user: String,
    pub files: u32,
    pub size_bytes: u64,
}

/// Every save folder on the console.
pub fn list(host: &Host) -> Result<Vec<SaveEntry>> {
    let mut conn = ftp::connect(host)?;
    let mut out = Vec::new();
    let users = ftp::list(&mut conn, "/user/home").unwrap_or_default();
    for user in users.iter().filter(|e| e.is_dir) {
        let dir = format!("/user/home/{}/savedata", user.name);
        let Ok(entries) = ftp::list(&mut conn, &dir) else {
            continue;
        };
        for entry in entries.iter().filter(|e| e.is_dir) {
            let (files, size_bytes) = folder_stats(&mut conn, &entry.path);
            out.push(SaveEntry {
                title_id: entry.name.clone(),
                title: crate::library::read_meta(&mut conn, &entry.name)
                    .and_then(|m| m.title),
                path: entry.path.clone(),
                user: user.name.clone(),
                files,
                size_bytes,
            });
        }
    }
    let _ = conn.quit();
    out.sort_by(|a, b| {
        a.title
            .as_deref()
            .unwrap_or(&a.title_id)
            .to_lowercase()
            .cmp(&b.title.as_deref().unwrap_or(&b.title_id).to_lowercase())
    });
    Ok(out)
}

fn folder_stats(conn: &mut suppaftp::FtpStream, path: &str) -> (u32, u64) {
    let Ok(entries) = ftp::list(conn, path) else {
        return (0, 0);
    };
    let mut files = 0;
    let mut size = 0;
    for entry in entries {
        if entry.is_dir {
            let (nested, nested_size) = folder_stats(conn, &entry.path);
            files += nested;
            size += nested_size;
        } else {
            files += 1;
            size += entry.size_bytes;
        }
    }
    (files, size)
}

/// Zip one or more saves into `out`.
pub fn backup(host: &Host, saves: &[SaveEntry], out: &Path, ctx: &JobContext) -> Result<u64> {
    if saves.is_empty() {
        return Err(Error::Invalid("no saves selected".into()));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut conn = ftp::connect(host)?;
    let total: u64 = saves.iter().map(|s| s.size_bytes).sum();
    let mut done = 0u64;
    for save in saves {
        ctx.message(format!("backing up {}", save.title.as_deref().unwrap_or(&save.title_id)));
        let files = collect_files(&mut conn, &save.path);
        for remote in files {
            ctx.check_cancel()?;
            let rel = remote
                .strip_prefix(&format!("{}/", save.path))
                .unwrap_or(&remote)
                .to_string();
            let name = format!("{}/{}", save.title_id, rel);
            let Ok(head) = ftp::read_head(&mut conn, &remote, 32 * 1024 * 1024) else {
                continue;
            };
            zip.start_file(name, options)?;
            zip.write_all(&head)?;
            done += head.len() as u64;
            ctx.progress(done, total.max(done));
        }
    }
    zip.finish()?;
    let _ = conn.quit();
    Ok(done)
}

fn collect_files(conn: &mut suppaftp::FtpStream, dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = ftp::list(conn, dir) else {
        return out;
    };
    for entry in entries {
        if entry.is_dir {
            out.extend(collect_files(conn, &entry.path));
        } else {
            out.push(entry.path);
        }
    }
    out
}

/// Upload a backup zip back to the console. Every entry's top folder is the
/// title id it restores into.
pub fn restore(host: &Host, zip_path: &Path, ctx: &JobContext) -> Result<u64> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut conn = ftp::connect(host)?;
    let users = ftp::list(&mut conn, "/user/home").unwrap_or_default();
    let user = users
        .iter()
        .find(|e| e.is_dir)
        .map(|e| e.name.clone())
        .ok_or_else(|| Error::NotFound("no user home on the console".into()))?;
    let total: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.size()))
        .sum();
    let mut done = 0u64;
    for i in 0..archive.len() {
        ctx.check_cancel()?;
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let rel = name.to_string_lossy().replace('\\', "/");
        let Some((title_id, rest)) = rel.split_once('/') else {
            continue;
        };
        let remote = format!("/user/home/{user}/savedata/{title_id}/{rest}");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        ftp::upload_bytes(&mut conn, &bytes, &remote, ctx)?;
        done += bytes.len() as u64;
        ctx.progress(done, total.max(done));
    }
    let _ = conn.quit();
    Ok(done)
}

/// Delete one save folder.
pub fn delete(host: &Host, path: &str) -> Result<()> {
    let mut conn = ftp::connect(host)?;
    let result = ftp::remove(&mut conn, path, true);
    let _ = conn.quit();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_requires_a_selection() {
        let host = Host::new("test", "127.0.0.1");
        let manager = crate::transfer::JobManager::new();
        let (tx, rx) = std::sync::mpsc::channel();
        manager.spawn(
            "backup",
            "saves",
            |_| {},
            move |ctx| {
                let result = backup(&host, &[], Path::new("/tmp/none.zip"), ctx);
                tx.send(result.is_err()).unwrap();
                Ok(())
            },
        );
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap());
    }
}
