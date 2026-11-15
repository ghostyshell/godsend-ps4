//! Getting things onto the console: packages go through GoldHEN's remote
//! installer (or land in `/user/pkg`), payloads go through the bin loader, and
//! archives are unpacked first.
//!
//! A PS4 installs from PKG files, not from folders: the console's own database
//! registers a title when the package installer runs. The app therefore never
//! copies an app folder into `/user/app` (that would produce an entry the
//! console cannot launch) and says so instead.

use crate::config::{Config, Host};
use crate::error::{Error, Result};
use crate::ftp;
use crate::pkg;
use crate::rpi;
use crate::split;
use crate::transfer::{dir_size, JobContext};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const HOMEBREW_DIR: &str = "/user/homebrew";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalKind {
    Pkg,
    /// A payload: `.elf` or `.bin` sent to the bin loader.
    Payload,
    /// An unpacked package tree (a converted folder).
    Folder,
    Zip,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallReport {
    pub remote_path: String,
    pub kind: String,
    pub bytes: u64,
    /// True when the console's installer was asked to install the package.
    pub install_requested: bool,
    pub message: String,
}

/// What kind of local content this path is.
pub fn classify(path: &Path) -> LocalKind {
    if path.is_dir() {
        return if is_package_tree(path) {
            LocalKind::Folder
        } else {
            LocalKind::Other
        };
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "pkg" => LocalKind::Pkg,
        "elf" | "bin" => LocalKind::Payload,
        "zip" => LocalKind::Zip,
        _ => LocalKind::Other,
    }
}

/// An extracted package: `sce_sys/param.json` beside `eboot.bin`.
pub fn is_package_tree(path: &Path) -> bool {
    path.join("sce_sys/param.json").is_file() || path.join("eboot.bin").is_file()
}

/// Read a local file header without loading the whole file.
pub fn read_head(path: &Path, max: usize) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; max];
    let read = file.read(&mut buf)?;
    buf.truncate(read);
    Ok(buf)
}

/// Full install flow for one local path.
pub fn install_local(
    host: &Host,
    local: &Path,
    config: &Config,
    ctx: &JobContext,
) -> Result<InstallReport> {
    match classify(local) {
        LocalKind::Pkg => install_pkg(host, local, config, ctx),
        LocalKind::Payload => {
            let bytes = std::fs::read(local)?.len() as u64;
            ctx.message("sending payload to the bin loader");
            let reply = send_payload(host, local)?;
            Ok(InstallReport {
                remote_path: format!("{}:{}", host.addr, host.binloader_port),
                kind: "payload".into(),
                bytes,
                install_requested: true,
                message: if reply.is_empty() {
                    "payload sent".into()
                } else {
                    format!("payload sent: {reply}")
                },
            })
        }
        LocalKind::Zip => {
            ctx.message("unpacking archive");
            let unpacked = unpack_zip(local, ctx)?;
            let result = install_local(host, &unpacked, config, ctx);
            let _ = std::fs::remove_dir_all(&unpacked);
            result
        }
        LocalKind::Folder => Err(Error::Invalid(format!(
            "{} is an unpacked package tree; a PS4 installs PKG files. Build a PKG from it first.",
            local.display()
        ))),
        LocalKind::Other => Err(Error::Invalid(format!(
            "{} is not a PKG, payload or ZIP",
            local.display()
        ))),
    }
}

fn install_pkg(
    host: &Host,
    local: &Path,
    config: &Config,
    ctx: &JobContext,
) -> Result<InstallReport> {
    let size = std::fs::metadata(local)?.len();
    if config.auto_install_pkg && rpi::available(host) {
        ctx.message("installing through the console's package installer");
        let served = rpi::install(host, local, ctx)?;
        return Ok(InstallReport {
            remote_path: format!("{} (streamed)", host.rpi_base()),
            kind: "pkg".into(),
            bytes: served,
            install_requested: true,
            message: "package delivered; the console installs it in the background".into(),
        });
    }

    // No installer: park the package where GoldHEN looks for local packages.
    ctx.message("uploading to /user/pkg");
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("package has no file name".into()))?;
    let remote = format!("{}/{}", pkg::INSTALL_DIR, name);
    let mut conn = ftp::connect(host)?;
    let result = ftp::upload(&mut conn, local, &remote, ctx);
    let _ = conn.quit();
    result?;
    Ok(InstallReport {
        remote_path: remote.clone(),
        kind: "pkg".into(),
        bytes: size,
        install_requested: false,
        message: format!(
            "uploaded to {remote}; start GoldHEN's installer (or a payload) to install it"
        ),
    })
}

/// Send one payload file to GoldHEN's bin loader.
pub fn send_payload(host: &Host, local: &Path) -> Result<String> {
    let bytes = std::fs::read(local)?;
    if bytes.is_empty() {
        return Err(Error::Payload("the payload file is empty".into()));
    }
    let addr: SocketAddr = format!("{}:{}", host.addr, host.binloader_port)
        .parse()
        .map_err(|e| Error::Payload(format!("bad bin loader address: {e}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|e| Error::Payload(format!("bin loader at {addr}: {e}")))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(20)));
    stream
        .write_all(&bytes)
        .map_err(|e| Error::Payload(format!("send: {e}")))?;
    stream.flush().ok();
    let mut reply = Vec::new();
    let mut buf = [0u8; 256];
    if let Ok(read) = stream.read(&mut buf) {
        reply.extend_from_slice(&buf[..read]);
    }
    Ok(String::from_utf8_lossy(&reply).trim().to_string())
}

/// Upload one file, splitting it when the destination is a USB volume and the
/// file is above the split limit.
pub fn push_file(
    conn: &mut suppaftp::FtpStream,
    local: &Path,
    remote_dir: &str,
    split_bytes: u64,
    ctx: &JobContext,
) -> Result<String> {
    let size = std::fs::metadata(local)?.len();
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("file has no name".into()))?;
    let target = format!("{}/{}", remote_dir.trim_end_matches('/'), name);
    ftp::mkdir_all(conn, remote_dir)?;
    if remote_dir.starts_with("/mnt/") && split_bytes > 0 && size > split_bytes {
        ctx.message(format!("splitting {name} for FAT32"));
        let parts = split::split(local, split_bytes, ctx)?;
        for part in &parts {
            let part_name = part.file_name().unwrap_or_default().to_string_lossy();
            let remote = format!("{}/{}", remote_dir.trim_end_matches('/'), part_name);
            ftp::upload(conn, part, &remote, ctx)?;
            let _ = std::fs::remove_file(part);
        }
        return Ok(target);
    }
    ftp::upload(conn, local, &target, ctx)?;
    Ok(target)
}

/// Upload a folder tree under `remote_parent/<folder name>`.
pub fn push_folder(
    conn: &mut suppaftp::FtpStream,
    local: &Path,
    remote_parent: &str,
    ctx: &JobContext,
) -> Result<String> {
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("folder has no name".into()))?;
    let base = format!("{}/{}", remote_parent.trim_end_matches('/'), name);
    let total = dir_size(local);
    let mut done = 0u64;
    ctx.progress(0, total);
    ftp::mkdir_all(conn, &base)?;
    for (path, is_dir) in walk(local) {
        ctx.check_cancel()?;
        let rel = path.strip_prefix(local).unwrap_or(&path);
        let remote = format!("{}/{}", base, rel.to_string_lossy().replace('\\', "/"));
        if is_dir {
            ftp::mkdir_all(conn, &remote)?;
            continue;
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        ftp::upload(conn, &path, &remote, ctx)?;
        done += size;
        ctx.progress(done, total);
    }
    Ok(base)
}

/// Depth-first listing of a local tree, directories before their contents.
fn walk(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        out.push((path.clone(), is_dir));
        if is_dir {
            out.extend(walk(&path));
        }
    }
    out
}

/// Unpack a ZIP into a fresh temp directory and return the folder that looks
/// like the package root.
pub fn unpack_zip(zip_path: &Path, ctx: &JobContext) -> Result<PathBuf> {
    let dest = std::env::temp_dir().join(format!(
        "godsend-ps4-unzip-{}-{}",
        std::process::id(),
        stamp()
    ));
    std::fs::create_dir_all(&dest)?;
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let total: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.size()))
        .sum();
    let mut done = 0u64;
    for i in 0..archive.len() {
        ctx.check_cancel()?;
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        // macOS resource forks and Finder junk never belong on the console.
        let text = name.to_string_lossy();
        if text.starts_with("__MACOSX") || text.contains("/._") || text.ends_with(".DS_Store") {
            continue;
        }
        let out = dest.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut outfile = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut outfile)?;
        done += entry.size();
        ctx.progress(done, total);
    }
    Ok(find_package_root(&dest).unwrap_or(dest))
}

fn find_package_root(dir: &Path) -> Option<PathBuf> {
    if is_package_tree(dir) {
        return Some(dir.to_path_buf());
    }
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    let dirs: Vec<_> = entries.iter().filter(|e| e.path().is_dir()).collect();
    if dirs.len() == 1 {
        return find_package_root(&dirs[0].path());
    }
    for entry in entries {
        if entry.path().is_dir() {
            if let Some(found) = find_package_root(&entry.path()) {
                return Some(found);
            }
        }
    }
    None
}

fn stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}

/// Size of a folder, for the install preview.
pub fn folder_size(path: &Path) -> u64 {
    dir_size(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_local_paths() {
        let dir = std::env::temp_dir().join(format!("godsend-ps4-kind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sce_sys")).unwrap();
        std::fs::write(dir.join("sce_sys/param.json"), b"{}").unwrap();
        std::fs::write(dir.join("eboot.bin"), b"x").unwrap();
        assert_eq!(classify(&dir), LocalKind::Folder);
        std::fs::write(dir.join("game.pkg"), b"x").unwrap();
        assert_eq!(classify(&dir.join("game.pkg")), LocalKind::Pkg);
        std::fs::write(dir.join("payload.elf"), b"x").unwrap();
        assert_eq!(classify(&dir.join("payload.elf")), LocalKind::Payload);
        std::fs::write(dir.join("homebrew.zip"), b"x").unwrap();
        assert_eq!(classify(&dir.join("homebrew.zip")), LocalKind::Zip);
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert_eq!(classify(&dir.join("notes.txt")), LocalKind::Other);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
