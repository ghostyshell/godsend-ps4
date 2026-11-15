//! Tauri command layer over `godsend-core`.
//!
//! The frontend calls these with `invoke(...)`. Long work runs in the job
//! queue and reports through `job://progress` and `job://job`.

use godsend_core::config::{Config, Host};
use godsend_core::convert;
use godsend_core::device;
use godsend_core::doctor;
use godsend_core::fm;
use godsend_core::homebrew::{self, CatalogEntry};
use godsend_core::install;
use godsend_core::library::{self, GameEntry, LibraryView};
use godsend_core::pad;
use godsend_core::saves::{self, SaveEntry};
use godsend_core::sources::{self, GameHit, IaFile, IaItem};
use godsend_core::store;
use godsend_core::transfer::{set_progress_hook, Job, JobContext, JobManager};
use godsend_core::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    jobs: JobManager,
    config: Mutex<Config>,
}

impl AppState {
    fn new() -> Self {
        let jobs = JobManager::new();
        jobs.load_journal();
        Self {
            jobs,
            config: Mutex::new(Config::load()),
        }
    }

    fn config(&self) -> Config {
        self.config.lock().expect("config lock").clone()
    }

    fn host(&self) -> Result<Host> {
        self.config().require_active()
    }

    fn replace_config(&self, next: Config) -> Result<Config> {
        next.save()?;
        *self.config.lock().expect("config lock") = next.clone();
        Ok(next)
    }
}

#[derive(Serialize)]
struct Snapshot {
    hosts: Vec<Host>,
    active: Option<String>,
    config: Config,
    doctor: Vec<doctor::Check>,
    statuses: Vec<device::DeviceStatus>,
    pad: Option<pad::PadStatus>,
    jobs: Vec<Job>,
}

#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> Snapshot {
    let config = state.config();
    let statuses = device::probe_all(&config.hosts);
    let pad = config.active().map(|host| pad::status(&host));
    Snapshot {
        hosts: config.hosts.clone(),
        active: config.active_host.clone(),
        doctor: doctor::run(&config),
        statuses,
        pad,
        jobs: state.jobs.list(),
        config,
    }
}

#[tauri::command]
fn probe_host(state: State<'_, AppState>, id: Option<String>) -> Result<device::DeviceStatus> {
    let config = state.config();
    let host = match id {
        Some(id) => config
            .hosts
            .into_iter()
            .find(|h| h.id == id)
            .ok_or_else(|| Error::NotFound(format!("no host {id}")))?,
        None => config.require_active()?,
    };
    Ok(device::probe(&host))
}

#[tauri::command]
fn probe_all(state: State<'_, AppState>) -> Vec<device::DeviceStatus> {
    device::probe_all(&state.config().hosts)
}

#[tauri::command]
fn discover(subnet: String) -> Vec<Host> {
    let subnet = if subnet.trim().is_empty() {
        auto_subnet()
    } else {
        subnet.trim().to_string()
    };
    device::discover(&subnet)
}

fn auto_subnet() -> String {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(socket) = socket {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                let parts: Vec<String> = addr.ip().to_string().split('.').map(|s| s.to_string()).collect();
                if parts.len() == 4 {
                    return format!("{}.{}.{}", parts[0], parts[1], parts[2]);
                }
            }
        }
    }
    "192.168.1".into()
}

#[tauri::command]
fn host_add(state: State<'_, AppState>, host: Host) -> Result<Config> {
    let mut config = state.config();
    config.upsert_host(host);
    state.replace_config(config)
}

#[tauri::command]
fn host_remove(state: State<'_, AppState>, id: String) -> Result<Config> {
    let mut config = state.config();
    config.remove_host(&id);
    state.replace_config(config)
}

#[tauri::command]
fn host_set_active(state: State<'_, AppState>, id: String) -> Result<Config> {
    let mut config = state.config();
    if !config.hosts.iter().any(|h| h.id == id) {
        return Err(Error::NotFound(format!("no host {id}")));
    }
    config.active_host = Some(id);
    state.replace_config(config)
}

#[tauri::command]
fn library_scan(state: State<'_, AppState>) -> Result<LibraryView> {
    library::scan(&state.host()?)
}

#[tauri::command]
fn icon(state: State<'_, AppState>, path: String) -> Option<String> {
    let host = state.host().ok()?;
    let bytes = library::icon_bytes(&host, &path)?;
    Some(format!("data:image/png;base64,{}", base64_encode(&bytes)))
}

#[tauri::command]
fn library_delete(state: State<'_, AppState>, path: String, is_dir: bool) -> Result<()> {
    library::delete_entry(&state.host()?, &path, is_dir)
}

#[tauri::command]
fn send_payload(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<u64> {
    let host = state.host()?;
    let local = PathBuf::from(&path);
    let label = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    Ok(spawn_job(app, state.inner(), "payload", &label, move |ctx| {
        ctx.message("sending payload to the bin loader");
        let reply = install::send_payload(&host, &local)?;
        ctx.message(if reply.is_empty() {
            "payload sent".to_string()
        } else {
            format!("payload sent: {reply}")
        });
        Ok(())
    }))
}

#[tauri::command]
fn pad_status(state: State<'_, AppState>) -> Result<pad::PadStatus> {
    Ok(pad::status(&state.host()?))
}

/// Exchange a WebRTC offer with the pad payload. The answer carries the
/// payload's ICE candidate; the webview builds the peer connection.
#[tauri::command]
fn pad_offer(state: State<'_, AppState>, offer: String) -> Result<String> {
    pad::exchange(&state.host()?, &offer)
}

#[tauri::command]
fn homebrew_catalog(refresh: bool) -> Vec<CatalogEntry> {
    homebrew::catalog(refresh)
}

#[tauri::command]
fn homebrew_install(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<u64> {
    let host = state.host()?;
    let config = state.config();
    let entry = homebrew::catalog(false)
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| Error::NotFound(format!("no homebrew {id}; refresh the catalog")))?;
    Ok(spawn_job(
        app,
        state.inner(),
        "homebrew",
        &entry.name.clone(),
        move |ctx| {
            let report = homebrew::install(&host, &config, &entry, ctx)?;
            ctx.message(report.message);
            Ok(())
        },
    ))
}

#[tauri::command]
fn store_search(
    state: State<'_, AppState>,
    query: String,
    source: String,
) -> Result<serde_json::Value> {
    let config = state.config();
    if source == "ia" {
        let items = sources::ia_search(&query, 30)?;
        Ok(serde_json::json!({ "kind": "ia", "items": items }))
    } else {
        let hits = sources::gamesearch(&config.gamesearch_base, &query, 40)?;
        Ok(serde_json::json!({ "kind": "gamesearch", "games": hits }))
    }
}

#[tauri::command]
fn store_files(item: String) -> Result<Vec<IaFile>> {
    sources::ia_item_files(&item)
}

#[tauri::command]
fn store_download(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    id: String,
    name: Option<String>,
) -> Result<u64> {
    let config = state.config();
    if source == "ia" {
        let files = sources::ia_item_files(&id)?;
        let file = match name {
            Some(wanted) => files
                .into_iter()
                .find(|f| f.name == wanted)
                .ok_or_else(|| Error::NotFound(format!("no file {wanted} in {id}")))?,
            None => {
                let mut list = files;
                match list
                    .iter()
                    .position(|f| f.name.to_lowercase().ends_with(".pkg"))
                {
                    Some(index) => list.swap_remove(index),
                    None => list
                        .into_iter()
                        .next()
                        .ok_or_else(|| Error::NotFound(format!("no downloadable file in {id}")))?,
                }
            }
        };
        let label = file.name.clone();
        return Ok(spawn_job(
            app,
            state.inner(),
            "download",
            &label,
            move |ctx| {
                let path = store::download_url(&file.url, &file.name, &config, ctx)?;
                ctx.message(format!("downloaded to {}", path.display()));
                Ok(())
            },
        ));
    }

    let detail = sources::gamesearch_detail(&config.gamesearch_base, &id).map_err(|e| {
        Error::Http(format!(
            "could not load links for {id}: {e}. Set the game-search backend (Go service) in Settings; the public site hides links behind sign-in."
        ))
    })?;
    let label = detail.title.clone();
    Ok(spawn_job(
        app,
        state.inner(),
        "download",
        &label,
        move |ctx| {
            let path = store::download_hit(&detail, &config, ctx)?;
            ctx.message(format!("downloaded to {}", path.display()));
            Ok(())
        },
    ))
}

#[derive(Serialize)]
struct DownloadsView {
    dir: String,
    files: Vec<fm::FmEntry>,
}

#[tauri::command]
fn downloads_list(state: State<'_, AppState>) -> Result<DownloadsView> {
    let config = state.config();
    let dir = godsend_core::aria2::download_dir(&config);
    let files = fm::local_list(&dir).unwrap_or_default();
    Ok(DownloadsView {
        dir: dir.display().to_string(),
        files,
    })
}

#[tauri::command]
fn download_delete(name: String) -> Result<()> {
    let config = Config::load();
    let dir = godsend_core::aria2::download_dir(&config);
    let path = safe_child(&dir, &name)?;
    fm::delete_local(&path)
}

#[tauri::command]
fn download_open(name: String) -> Result<String> {
    let config = Config::load();
    let dir = godsend_core::aria2::download_dir(&config);
    let path = safe_child(&dir, &name)?;
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }
    reveal_in_finder(&path)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn download_push(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<u64> {
    let host = state.host()?;
    let config = state.config();
    let dir = godsend_core::aria2::download_dir(&config);
    let path = safe_child(&dir, &name)?;
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }
    Ok(spawn_job(
        app,
        state.inner(),
        "install",
        &name,
        move |ctx| {
            let report = install::install_local(&host, &path, &config, ctx)?;
            ctx.message(report.message);
            Ok(())
        },
    ))
}

#[tauri::command]
fn install_path(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<u64> {
    let host = state.host()?;
    let config = state.config();
    let local = PathBuf::from(&path);
    let label = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    Ok(spawn_job(
        app,
        state.inner(),
        "install",
        &label,
        move |ctx| {
            let report = install::install_local(&host, &local, &config, ctx)?;
            ctx.message(report.message);
            Ok(())
        },
    ))
}

#[tauri::command]
fn fm_local(path: String) -> Result<Vec<fm::FmEntry>> {
    fm::local_list(&PathBuf::from(path))
}

#[tauri::command]
fn fm_remote(state: State<'_, AppState>, path: String) -> Result<Vec<fm::FmEntry>> {
    fm::remote_list(&state.host()?, &path)
}

#[tauri::command]
fn fm_mkdir(path: String, name: String) -> Result<String> {
    let created = fm::mkdir_local(&PathBuf::from(path), &name)?;
    Ok(created.display().to_string())
}

#[tauri::command]
fn fm_mkdir_remote(state: State<'_, AppState>, path: String) -> Result<()> {
    fm::mkdir_remote(&state.host()?, &path)
}

#[tauri::command]
fn fm_delete(path: String, is_dir: bool, remote: bool, state: State<'_, AppState>) -> Result<()> {
    if remote {
        fm::delete_remote(&state.host()?, &path, is_dir)
    } else {
        fm::delete_local(&PathBuf::from(path))
    }
}

#[tauri::command]
fn fm_upload(
    app: AppHandle,
    state: State<'_, AppState>,
    local: String,
    remote_dir: String,
) -> Result<u64> {
    let host = state.host()?;
    let split_bytes = state.config().split_bytes;
    let label = local.rsplit('/').next().unwrap_or(&local).to_string();
    Ok(spawn_job(
        app,
        state.inner(),
        "upload",
        &label,
        move |ctx| {
            let path = fm::upload(
                &host,
                &PathBuf::from(local),
                &remote_dir,
                split_bytes,
                ctx,
            )?;
            ctx.message(format!("uploaded to {path}"));
            Ok(())
        },
    ))
}

#[tauri::command]
fn fm_download(
    app: AppHandle,
    state: State<'_, AppState>,
    remote: String,
    local_dir: String,
) -> Result<u64> {
    let host = state.host()?;
    let label = remote.rsplit('/').next().unwrap_or(&remote).to_string();
    Ok(spawn_job(
        app,
        state.inner(),
        "download",
        &label,
        move |ctx| {
            let path = fm::download(&host, &remote, &PathBuf::from(local_dir), ctx)?;
            ctx.message(format!("saved to {}", path.display()));
            Ok(())
        },
    ))
}

#[tauri::command]
fn convert_inspect(path: String) -> Result<convert::ConvertInfo> {
    convert::inspect(&PathBuf::from(path))
}

#[tauri::command]
fn convert_run(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    src: String,
    out: String,
    chunk_mb: Option<u64>,
) -> Result<u64> {
    let src_path = PathBuf::from(&src);
    let out_path = PathBuf::from(&out);
    let label = kind.clone();
    Ok(spawn_job(
        app,
        state.inner(),
        "convert",
        &label,
        move |ctx| match kind.as_str() {
            "pkg_extract" => convert::pkg_extract(&src_path, &out_path, ctx).map(|_| ()),
            "split" => {
                let chunk = chunk_mb.unwrap_or(4095) * 1024 * 1024;
                convert::split_file(&src_path, chunk, ctx).map(|_| ())
            }
            "merge" => convert::merge_file(&src_path, &out_path, ctx).map(|_| ()),
            other => Err(Error::Invalid(format!("unknown conversion {other}"))),
        },
    ))
}

#[tauri::command]
fn saves_list(state: State<'_, AppState>) -> Result<Vec<SaveEntry>> {
    saves::list(&state.host()?)
}

#[tauri::command]
fn saves_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    folders: Vec<String>,
) -> Result<u64> {
    let host = state.host()?;
    let config = state.config();
    let label = format!("{} save(s)", folders.len());
    Ok(spawn_job(
        app,
        state.inner(),
        "backup",
        &label,
        move |ctx| {
            let all = saves::list(&host)?;
            let selected: Vec<_> = all
                .into_iter()
                .filter(|s| folders.iter().any(|f| f == &s.title_id))
                .collect();
            let dir = godsend_core::aria2::download_dir(&config);
            std::fs::create_dir_all(&dir)?;
            let stamp = now_stamp();
            let out = dir.join(format!("ps4-saves-{stamp}.zip"));
            let bytes = saves::backup(&host, &selected, &out, ctx)?;
            ctx.message(format!(
                "{} saved to {}",
                doctor::human_bytes(bytes),
                out.display()
            ));
            Ok(())
        },
    ))
}

#[tauri::command]
fn saves_restore(app: AppHandle, state: State<'_, AppState>, zip: String) -> Result<u64> {
    let host = state.host()?;
    let path = PathBuf::from(&zip);
    let label = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| zip.clone());
    Ok(spawn_job(
        app,
        state.inner(),
        "restore",
        &label,
        move |ctx| {
            let bytes = saves::restore(&host, &path, ctx)?;
            ctx.message(format!("restored {}", doctor::human_bytes(bytes)));
            Ok(())
        },
    ))
}

#[tauri::command]
fn saves_delete(state: State<'_, AppState>, path: String) -> Result<()> {
    saves::delete(&state.host()?, &path)
}

#[tauri::command]
fn config_get(state: State<'_, AppState>) -> Config {
    state.config()
}

#[tauri::command]
fn config_set(state: State<'_, AppState>, patch: serde_json::Value) -> Result<Config> {
    let mut config = state.config();
    config.patch_from_json(&patch)?;
    *state.config.lock().expect("config lock") = config.clone();
    Ok(config)
}

#[tauri::command]
fn doctor_run(state: State<'_, AppState>) -> Vec<doctor::Check> {
    doctor::run(&state.config())
}

#[tauri::command]
fn jobs(state: State<'_, AppState>) -> Vec<Job> {
    state.jobs.list()
}

#[tauri::command]
fn job_cancel(state: State<'_, AppState>, id: u64) -> bool {
    state.jobs.cancel(id)
}

#[tauri::command]
fn jobs_clear(state: State<'_, AppState>) {
    state.jobs.clear_finished();
}

#[tauri::command]
fn reveal(path: String) -> Result<()> {
    reveal_in_finder(&PathBuf::from(path))
}

#[tauri::command]
fn open_data_dir() -> Result<String> {
    let dir = godsend_core::config::data_dir();
    std::fs::create_dir_all(&dir)?;
    reveal_in_finder(&dir)?;
    Ok(dir.display().to_string())
}

fn spawn_job<F>(app: AppHandle, state: &AppState, kind: &str, label: &str, work: F) -> u64
where
    F: FnOnce(&JobContext) -> Result<()> + Send + 'static,
{
    let handle = app.clone();
    state.jobs.spawn(
        kind,
        label,
        move |job| {
            let _ = handle.emit("job://job", job);
        },
        work,
    )
}

fn now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}

fn safe_child(dir: &Path, name: &str) -> Result<PathBuf> {
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(Error::Invalid(format!("'{name}' is not a file name")));
    }
    Ok(dir.join(name))
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn reveal_in_finder(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }
    #[cfg(target_os = "macos")]
    let ok = std::process::Command::new("open").arg(path).spawn().is_ok();
    #[cfg(target_os = "windows")]
    let ok = std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .is_ok();
    #[cfg(all(unix, not(target_os = "macos")))]
    let ok = std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .is_ok();
    if ok {
        Ok(())
    } else {
        Err(Error::Other("could not open the file manager".into()))
    }
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "godsend_core=info".into()),
        )
        .init();
    let _ = godsend_core::config::ensure_dirs();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .setup(|app| {
            // The bundled MCP server starts with the app so an agent can connect
            // while the window is open. Loopback only, no auth.
            let mcp_port = Config::load().mcp_port;
            if mcp_port > 0 {
                std::thread::spawn(move || {
                    if let Err(err) = godsend_ps4_mcp::serve_http(mcp_port) {
                        tracing::warn!("MCP server on 127.0.0.1:{mcp_port} failed: {err}");
                    }
                });
            }
            let handle = app.handle().clone();
            set_progress_hook(Some(Arc::new(move |job_id, done, total| {
                let _ = handle.emit(
                    "job://progress",
                    serde_json::json!({ "id": job_id, "done": done, "total": total }),
                );
            })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            probe_host,
            probe_all,
            discover,
            host_add,
            host_remove,
            host_set_active,
            library_scan,
            icon,
            library_delete,
            send_payload,
            pad_status,
            pad_offer,
            homebrew_catalog,
            homebrew_install,
            store_search,
            store_files,
            store_download,
            install_path,
            downloads_list,
            download_delete,
            download_open,
            download_push,
            fm_local,
            fm_remote,
            fm_mkdir,
            fm_mkdir_remote,
            fm_delete,
            fm_upload,
            fm_download,
            convert_inspect,
            convert_run,
            saves_list,
            saves_backup,
            saves_restore,
            saves_delete,
            config_get,
            config_set,
            doctor_run,
            jobs,
            job_cancel,
            jobs_clear,
            reveal,
            open_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GODsend PS4");
}

#[allow(unused)]
fn _keep_types(_: GameEntry, _: GameHit, _: IaItem, _: Job) {}
