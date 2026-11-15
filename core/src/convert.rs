//! Local conversions for PS4 content.
//!
//! The console installs PKG files. What a desktop can usefully do around that:
//! unpack a PKG to inspect or patch it, split a package that will not fit on a
//! FAT32 drive, and merge a split set back into one file. Building a PKG from a
//! folder needs Sony's package signing, so the app points at a PKG tool instead
//! of pretending.

use crate::error::{Error, Result};
use crate::install;
use crate::pkg;
use crate::split;
use crate::transfer::JobContext;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ConvertInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    /// `pkg` | `folder` | `zip` | `split` | `payload` | `other`
    pub kind: String,
    /// `ps4` | `pkg` | `unknown`
    pub system: String,
    pub title_id: Option<String>,
    /// Number of parts for a split set.
    pub parts: usize,
}

/// Inspect any local path the Convert page can act on.
pub fn inspect(path: &Path) -> Result<ConvertInfo> {
    let metadata = std::fs::metadata(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut info = ConvertInfo {
        path: path.to_string_lossy().to_string(),
        name,
        size_bytes: metadata.len(),
        kind: "other".into(),
        system: "unknown".into(),
        title_id: None,
        parts: 0,
    };

    // A `.0` file is the head of a split set: report the joined size.
    let parts = split::find_parts(path);
    if parts.len() > 1 {
        info.kind = "split".into();
        info.parts = parts.len();
        info.size_bytes = parts
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
            .sum();
        let head = split::read_head(&parts, 4096).unwrap_or_default();
        if head.len() >= 4 && head[0..4] == pkg::MAGIC {
            info.system = "pkg".into();
        }
        return Ok(info);
    }

    if path.is_dir() {
        info.kind = "folder".into();
        if install::is_package_tree(path) {
            info.system = "ps4".into();
            if let Ok(body) = std::fs::read_to_string(path.join("sce_sys/param.json")) {
                let meta = crate::param::parse(&body);
                info.title_id = meta.title_id;
            }
        }
        return Ok(info);
    }

    let lower = info.name.to_lowercase();
    if lower.ends_with(".pkg") {
        info.kind = "pkg".into();
        info.system = "pkg".into();
        if let Ok(parsed) = pkg::info(path) {
            info.title_id = parsed.title_id;
        }
    } else if lower.ends_with(".elf") || lower.ends_with(".bin") {
        info.kind = "payload".into();
    } else if lower.ends_with(".zip") {
        info.kind = "zip".into();
    }
    Ok(info)
}

/// Unpack a PKG to a folder.
pub fn pkg_extract(src: &Path, out: &Path, ctx: &JobContext) -> Result<u64> {
    pkg::extract(src, out, ctx)
}

pub fn split_file(src: &Path, chunk: u64, ctx: &JobContext) -> Result<Vec<PathBuf>> {
    ctx.message(format!(
        "splitting into {} MiB parts",
        chunk / (1024 * 1024)
    ));
    split::split(src, chunk, ctx)
}

pub fn merge_file(base: &Path, out: &Path, ctx: &JobContext) -> Result<u64> {
    let parts = split::find_parts(base);
    if parts.is_empty() {
        return Err(Error::NotFound(format!(
            "no parts found for {} (expected {}.0, {}.1, ...)",
            base.display(),
            base.display(),
            base.display()
        )));
    }
    ctx.message(format!("merging {} parts", parts.len()));
    split::merge(&parts, out, ctx)
}

/// What the install button will do with this local path, for the UI preview.
pub fn install_destination(local: &Path) -> String {
    match install::classify(local) {
        install::LocalKind::Pkg => "console installer (or /user/pkg)".to_string(),
        install::LocalKind::Payload => "bin loader (port 9090)".to_string(),
        install::LocalKind::Folder => "-".to_string(),
        install::LocalKind::Zip => "(unpacked, then its contents)".to_string(),
        install::LocalKind::Other => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_an_unpacked_tree() {
        let dir = std::env::temp_dir().join(format!("godsend-ps4-inspect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sce_sys")).unwrap();
        std::fs::write(
            dir.join("sce_sys/param.json"),
            br#"{"titleId":"CUSA07410","titleName":"God of War"}"#,
        )
        .unwrap();
        std::fs::write(dir.join("eboot.bin"), b"x").unwrap();
        let info = inspect(&dir).unwrap();
        assert_eq!(info.kind, "folder");
        assert_eq!(info.system, "ps4");
        assert_eq!(info.title_id.as_deref(), Some("CUSA07410"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_destination_matches_the_console_layout() {
        let dir = std::env::temp_dir().join(format!("godsend-ps4-dest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pkg = dir.join("a.pkg");
        std::fs::write(&pkg, b"x").unwrap();
        assert!(install_destination(&pkg).contains("installer"));
        let elf = dir.join("b.elf");
        std::fs::write(&elf, b"x").unwrap();
        assert!(install_destination(&elf).contains("bin loader"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
