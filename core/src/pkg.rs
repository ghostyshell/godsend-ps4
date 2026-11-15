//! PS4 PKG files: identify them and unpack them to a folder.
//!
//! A PKG starts with the magic `\x7fCNT`, then a 0x1000-byte header holding
//! the content id (`UP0001-CUSA07410_00-...`), and a table of entries plus an
//! embedded PFS image. Extraction is delegated to `orbis-pkg-util`, which
//! handles the PFS decryption; this module only supplies the header scan used
//! on remote files (where fetching 30 GB is not an option) and the job plumbing.

use crate::error::{Error, Result};
use crate::transfer::JobContext;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAGIC: [u8; 4] = [0x7f, 0x43, 0x4e, 0x54]; // "\x7fCNT"
pub const INSTALL_DIR: &str = "/user/pkg";

#[derive(Debug, Clone, Serialize)]
pub struct PkgInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub magic_ok: bool,
    /// Raw package type word at 0x04.
    pub pkg_type: u32,
    /// Nine-character title id (`CUSA07410`) from the content id.
    pub title_id: Option<String>,
    /// Full content id.
    pub content_id: Option<String>,
}

pub fn is_pkg(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("pkg"))
        .unwrap_or(false)
}

/// Inspect a local PKG using its first 64 KiB plus the file size.
pub fn info(path: &Path) -> Result<PkgInfo> {
    let size = std::fs::metadata(path)?.len();
    let head = crate::install::read_head(path, 64 * 1024)?;
    Ok(info_from(path.to_string_lossy().to_string(),
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        size,
        &head,
    ))
}

pub fn info_from(path: String, name: String, size_bytes: u64, head: &[u8]) -> PkgInfo {
    let parsed = from_head(head);
    PkgInfo {
        path,
        name,
        size_bytes,
        magic_ok: parsed.magic_ok,
        pkg_type: parsed.pkg_type,
        title_id: parsed.title_id,
        content_id: parsed.content_id,
    }
}

/// The fields a remote header read can recover.
#[derive(Debug, Clone, Default)]
pub struct HeadInfo {
    pub magic_ok: bool,
    pub pkg_type: u32,
    pub title_id: Option<String>,
    pub content_id: Option<String>,
}

/// Scan a header fragment for the PKG magic and content id.
pub fn from_head(head: &[u8]) -> HeadInfo {
    let magic_ok = head.len() >= 4 && head[0..4] == MAGIC;
    let pkg_type = if head.len() >= 8 {
        u32::from_be_bytes([head[4], head[5], head[6], head[7]])
    } else {
        0
    };
    let (content_id, title_id) = find_content_id(head);
    HeadInfo {
        magic_ok,
        pkg_type,
        title_id,
        content_id,
    }
}

/// Content ids look like `UP0001-CUSA07410_00-XXXXXXXXXXXXXXXXXX`.
fn find_content_id(head: &[u8]) -> (Option<String>, Option<String>) {
    let mut run = Vec::new();
    for &byte in head.iter().chain(std::iter::once(&0u8)) {
        if byte.is_ascii_graphic() {
            run.push(byte);
            continue;
        }
        if run.len() >= 24 {
            if let Ok(text) = std::str::from_utf8(&run) {
                if let Some(id) = crate::param::title_id_from_content_id(text) {
                    let id = id.strip_suffix('_').unwrap_or(&id).to_string();
                    return (Some(text.to_string()), Some(id));
                }
            }
        }
        run.clear();
    }
    // Fall back to a bare 9-character title id in the header.
    for window in head.windows(9) {
        if let Ok(text) = std::str::from_utf8(window) {
            if crate::param::is_title_id(text) {
                return (None, Some(text.to_string()));
            }
        }
    }
    (None, None)
}

/// Unpack a PKG into `out`, reporting whole-file progress.
pub fn extract(src: &Path, out: &Path, ctx: &JobContext) -> Result<u64> {
    if !is_pkg(src) {
        return Err(Error::Invalid(format!("{} is not a PKG", src.display())));
    }
    ctx.message("reading the package table");
    // Safety: the app owns the file for the duration of the call and nothing
    // else writes to it. The map is dropped when the function returns.
    let pkg = unsafe { orbis_pkg_util::open_pkg(src) }
        .map_err(|e| Error::Invalid(format!("open {}: {e}", src.display())))?;
    ctx.message(format!(
        "extracting {} entries",
        pkg.header().entry_count()
    ));
    std::fs::create_dir_all(out)?;
    let progress = JobProgress::new(ctx);
    let extractor = orbis_pkg_util::PkgExtractor::new(&pkg, progress, false);
    extractor
        .extract(out)
        .map_err(|e| Error::Invalid(format!("extract {}: {e}", src.display())))?;
    Ok(crate::transfer::dir_size(out))
}

/// Bridges extraction updates into the job queue.
struct JobProgress<'a> {
    ctx: &'a JobContext,
    total: AtomicU64,
    done: AtomicU64,
}

impl<'a> JobProgress<'a> {
    fn new(ctx: &'a JobContext) -> Self {
        Self {
            ctx,
            total: AtomicU64::new(0),
            done: AtomicU64::new(0),
        }
    }
}

impl orbis_pkg_util::ExtractProgress for JobProgress<'_> {
    fn entry_start(&self, path: &Path, current: usize, total: usize) {
        self.ctx
            .message(format!("entry {current}/{total}: {}", path.display()));
    }

    fn entry_skipped(&self, path: &Path, reason: &str) {
        self.ctx
            .message(format!("skipped {}: {reason}", path.display()));
    }

    fn pfs_start(&self, total_items: usize) {
        self.total.store(total_items as u64, Ordering::Relaxed);
        self.ctx.progress(0, total_items as u64);
    }

    fn pfs_file_completed(&self, _written: u64) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        self.ctx
            .progress(done, self.total.load(Ordering::Relaxed).max(done));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_content_id() {
        let mut head = vec![0u8; 512];
        head[0..4].copy_from_slice(&MAGIC);
        head[4..8].copy_from_slice(&1u32.to_be_bytes());
        head[0x30..0x30 + 36].copy_from_slice(b"UP0001-CUSA07410_00-0000000000000000");
        let info = from_head(&head);
        assert!(info.magic_ok);
        assert_eq!(info.pkg_type, 1);
        assert_eq!(info.title_id.as_deref(), Some("CUSA07410"));
        assert_eq!(
            info.content_id.as_deref(),
            Some("UP0001-CUSA07410_00-0000000000000000")
        );
    }

    #[test]
    fn reports_non_pkg_files() {
        let info = from_head(b"not a pkg");
        assert!(!info.magic_ok);
        assert_eq!(info.title_id, None);
    }
}
