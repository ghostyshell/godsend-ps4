//! GODsend PS4 command line.
//!
//! Every command talks to one configured console over FTP, with GoldHEN's
//! remote installer for packages and its bin loader for payloads.

use godsend_core::config::{Config, Host};
use godsend_core::convert;
use godsend_core::device;
use godsend_core::doctor;
use godsend_core::fm;
use godsend_core::homebrew;
use godsend_core::install;
use godsend_core::library;
use godsend_core::pad;
use godsend_core::pkg;
use godsend_core::saves;
use godsend_core::sources;
use godsend_core::store;
use godsend_core::transfer::JobManager;
use godsend_core::{Error, Result};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" || args[0] == "help" {
        usage();
        return;
    }
    if let Err(err) = run(&args) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn usage() {
    println!(
        r#"GODsend PS4 {version}

Usage: godsend-ps4 <command> [options]

Hosts
  hosts                          list configured consoles
  add-host <addr> [--name N] [--user U] [--pass P] [--ftp-port N] [--rpi-port N] [--pad-port N]
  rm-host <id>                   remove a console
  use <id>                       make a console active
  discover [--subnet 192.168.1]  scan the LAN for FTP servers on 2121 and 21
  status [<id>]                  probe FTP, package installer and virtual pad
  doctor                         preflight checks

Library and files
  scan                           list installed titles and waiting packages
  ls <remote-path>               list a console directory
  mkdir <remote-path>            create a remote directory
  rm <remote-path> [--dir]       delete a remote file or directory
  push <local> <remote-dir>      upload a file or folder
  pull <remote> [<local-dir>]    download a file or folder

Packages and payloads
  install <local>                install a PKG, send a payload, unpack a zip first
  pkg <file>                     show a package's title id and type
  extract <file.pkg> <out-dir>   unpack a package to a folder
  split <file> [--chunk-mb N]    split for a FAT32 USB drive
  merge <base> <out>             join <base>.0, <base>.1, ...

Saves
  saves                          list save folders
  backup <title-id...> [--out F] zip saves locally
  restore <backup.zip>           upload a backup back to the console

Downloads
  search <query>                 search the game index
  download <slug>                download a game's best link
  homebrew [--refresh]           list homebrew packages
  homebrew-install <id>          download and install one
  pad-status                     ask the virtual pad service what it supports
"#,
        version = godsend_core::VERSION
    );
}

fn run(args: &[String]) -> Result<()> {
    let mut config = Config::load();
    let cmd = args[0].as_str();
    match cmd {
        "hosts" => {
            for host in &config.hosts {
                let marker = if config.active_host.as_deref() == Some(&host.id) {
                    "*"
                } else {
                    " "
                };
                println!(
                    "{marker} {:<28} {:<20} ftp :{} rpi :{} pad :{}",
                    host.id, host.name, host.ftp_port, host.rpi_port, host.pad_port
                );
            }
            if config.hosts.is_empty() {
                println!("no consoles yet: godsend-ps4 add-host 192.168.1.50");
            }
            Ok(())
        }
        "add-host" => {
            let addr = args
                .get(1)
                .ok_or_else(|| Error::Invalid("add-host <addr>".into()))?;
            let name = flag_value(args, "--name").unwrap_or_else(|| addr.clone());
            let mut host = Host::new(&name, addr);
            if let Some(user) = flag_value(args, "--user") {
                host.ftp_user = user;
            }
            if let Some(pass) = flag_value(args, "--pass") {
                host.ftp_pass = pass;
            }
            if let Some(port) = flag_value(args, "--ftp-port").and_then(|v| v.parse().ok()) {
                host.ftp_port = port;
            }
            if let Some(port) = flag_value(args, "--rpi-port").and_then(|v| v.parse().ok()) {
                host.rpi_port = port;
            }
            if let Some(port) = flag_value(args, "--pad-port").and_then(|v| v.parse().ok()) {
                host.pad_port = port;
            }
            config.upsert_host(host.clone());
            config.save()?;
            println!("added {} ({})", host.name, host.id);
            Ok(())
        }
        "rm-host" => {
            let id = args
                .get(1)
                .ok_or_else(|| Error::Invalid("rm-host <id>".into()))?;
            config.remove_host(id);
            config.save()?;
            println!("removed {id}");
            Ok(())
        }
        "use" => {
            let id = args
                .get(1)
                .ok_or_else(|| Error::Invalid("use <id>".into()))?;
            if !config.hosts.iter().any(|h| &h.id == id) {
                return Err(Error::NotFound(format!("no host {id}")));
            }
            config.active_host = Some(id.clone());
            config.save()?;
            println!("active host: {id}");
            Ok(())
        }
        "discover" => {
            let subnet = flag_value(args, "--subnet").unwrap_or_else(auto_subnet);
            println!("scanning {subnet}.1-254 for FTP...");
            for host in device::discover(&subnet) {
                println!("found {} ({})", host.addr, host.id);
            }
            Ok(())
        }
        "status" => {
            let hosts = match args.get(1) {
                Some(id) => vec![config
                    .hosts
                    .iter()
                    .find(|h| &h.id == id || &h.addr == id)
                    .cloned()
                    .ok_or_else(|| Error::NotFound(format!("no host {id}")))?],
                None => config.hosts.clone(),
            };
            if hosts.is_empty() {
                return Err(Error::NoDevice);
            }
            for status in device::probe_all(&hosts) {
                println!(
                    "{} ({}) ftp={} {}ms installer={} pad={} titles={} packages={}",
                    status.name,
                    status.addr,
                    if status.ftp_ok { "up" } else { "down" },
                    status.ftp_ms,
                    if status.rpi_ok { "up" } else { "down" },
                    if status.pad_ok { "up" } else { "down" },
                    status.titles,
                    status.packages
                );
                if !status.message.is_empty() {
                    println!("  {}", status.message);
                }
            }
            Ok(())
        }
        "doctor" => {
            for check in doctor::run(&config) {
                println!("{} {} - {}", if check.ok { "ok  " } else { "fail" }, check.name, check.detail);
            }
            Ok(())
        }
        "scan" => {
            let view = library::scan(&config.require_active()?)?;
            for game in &view.games {
                println!(
                    "{:<40} {:<12} {:<10} {}",
                    game.title,
                    game.title_id.clone().unwrap_or_default(),
                    game.category,
                    game.path
                );
            }
            for package in &view.packages {
                println!(
                    "{:<40} {:<12} package    {}",
                    package.title,
                    package.title_id.clone().unwrap_or_default(),
                    package.path
                );
            }
            Ok(())
        }
        "ls" => {
            let path = args
                .get(1)
                .ok_or_else(|| Error::Invalid("ls <remote-path>".into()))?;
            for entry in fm::remote_list(&config.require_active()?, path)? {
                println!(
                    "{:>12} {} {}",
                    entry.size_bytes,
                    if entry.is_dir { "d" } else { "-" },
                    entry.name
                );
            }
            Ok(())
        }
        "mkdir" => {
            let path = args
                .get(1)
                .ok_or_else(|| Error::Invalid("mkdir <remote-path>".into()))?;
            fm::mkdir_remote(&config.require_active()?, path)?;
            println!("created {path}");
            Ok(())
        }
        "rm" => {
            let path = args
                .get(1)
                .ok_or_else(|| Error::Invalid("rm <remote-path>".into()))?;
            fm::delete_remote(&config.require_active()?, path, has_flag(args, "--dir"))?;
            println!("removed {path}");
            Ok(())
        }
        "push" => {
            let local = args
                .get(1)
                .cloned()
                .ok_or_else(|| Error::Invalid("push <local> <remote-dir>".into()))?;
            let remote = args
                .get(2)
                .cloned()
                .ok_or_else(|| Error::Invalid("push <local> <remote-dir>".into()))?;
            let host = config.require_active()?;
            let split_bytes = config.split_bytes;
            let label = local.clone();
            run_job(&label, move |ctx| {
                let path = fm::upload(&host, &PathBuf::from(&local), &remote, split_bytes, ctx)?;
                println!("uploaded to {path}");
                Ok(())
            })
        }
        "pull" => {
            let remote = args
                .get(1)
                .cloned()
                .ok_or_else(|| Error::Invalid("pull <remote> [<local-dir>]".into()))?;
            let local = args.get(2).cloned().unwrap_or_else(|| ".".to_string());
            let host = config.require_active()?;
            let label = remote.clone();
            run_job(&label, move |ctx| {
                let path = fm::download(&host, &remote, &PathBuf::from(local), ctx)?;
                println!("saved to {}", path.display());
                Ok(())
            })
        }
        "install" => {
            let local = args
                .get(1)
                .cloned()
                .ok_or_else(|| Error::Invalid("install <local>".into()))?;
            let host = config.require_active()?;
            let label = local.clone();
            run_job(&label, move |ctx| {
                let report = install::install_local(&host, &PathBuf::from(local), &config, ctx)?;
                println!("{}", report.message);
                Ok(())
            })
        }
        "pkg" => {
            let file = args
                .get(1)
                .ok_or_else(|| Error::Invalid("pkg <file>".into()))?;
            let info = pkg::info(&PathBuf::from(file))?;
            println!("magic:      {}", if info.magic_ok { "PKG" } else { "no" });
            println!("type:       0x{:08X}", info.pkg_type);
            println!("title id:   {}", info.title_id.unwrap_or_default());
            println!("content id: {}", info.content_id.unwrap_or_default());
            println!("size:       {}", doctor::human_bytes(info.size_bytes));
            Ok(())
        }
        "extract" => {
            let file = args
                .get(1)
                .ok_or_else(|| Error::Invalid("extract <file.pkg> <out-dir>".into()))?;
            let out = args
                .get(2)
                .ok_or_else(|| Error::Invalid("extract <file.pkg> <out-dir>".into()))?;
            let src = PathBuf::from(file);
            let dest = PathBuf::from(out);
            run_job(file, move |ctx| {
                let bytes = convert::pkg_extract(&src, &dest, ctx)?;
                println!("extracted {} to {}", doctor::human_bytes(bytes), dest.display());
                Ok(())
            })
        }
        "split" => {
            let file = args
                .get(1)
                .ok_or_else(|| Error::Invalid("split <file> [--chunk-mb N]".into()))?;
            let chunk = flag_value(args, "--chunk-mb")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(4095)
                * 1024
                * 1024;
            let src = PathBuf::from(file);
            run_job(file, move |ctx| {
                let parts = convert::split_file(&src, chunk, ctx)?;
                println!("wrote {} parts", parts.len());
                Ok(())
            })
        }
        "merge" => {
            let base = args
                .get(1)
                .ok_or_else(|| Error::Invalid("merge <base> <out>".into()))?;
            let out = args
                .get(2)
                .ok_or_else(|| Error::Invalid("merge <base> <out>".into()))?;
            let src = PathBuf::from(base);
            let dest = PathBuf::from(out);
            run_job(base, move |ctx| {
                let bytes = convert::merge_file(&src, &dest, ctx)?;
                println!("merged {} into {}", doctor::human_bytes(bytes), dest.display());
                Ok(())
            })
        }
        "saves" => {
            for save in saves::list(&config.require_active()?)? {
                println!(
                    "{:<12} {:<40} {} files {}",
                    save.title_id,
                    save.title.clone().unwrap_or_default(),
                    save.files,
                    doctor::human_bytes(save.size_bytes)
                );
            }
            Ok(())
        }
        "backup" => {
            let ids: Vec<String> = args[1..]
                .iter()
                .filter(|a| !a.starts_with("--"))
                .cloned()
                .collect();
            if ids.is_empty() {
                return Err(Error::Invalid("backup <title-id...> [--out F]".into()));
            }
            let host = config.require_active()?;
            let out = flag_value(args, "--out")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    godsend_core::aria2::download_dir(&config)
                        .join(format!("saves-{}.zip", ids.join("-")))
                });
            let label = "save backup".to_string();
            run_job(&label, move |ctx| {
                let all = saves::list(&host)?;
                let selected: Vec<_> = all
                    .into_iter()
                    .filter(|s| ids.iter().any(|id| id.eq_ignore_ascii_case(&s.title_id)))
                    .collect();
                let bytes = saves::backup(&host, &selected, &out, ctx)?;
                println!("wrote {} to {}", doctor::human_bytes(bytes), out.display());
                Ok(())
            })
        }
        "restore" => {
            let zip = args
                .get(1)
                .ok_or_else(|| Error::Invalid("restore <backup.zip>".into()))?;
            let host = config.require_active()?;
            let path = PathBuf::from(zip);
            let label = zip.clone();
            run_job(&label, move |ctx| {
                let bytes = saves::restore(&host, &path, ctx)?;
                println!("restored {}", doctor::human_bytes(bytes));
                Ok(())
            })
        }
        "search" => {
            let query = args
                .get(1)
                .ok_or_else(|| Error::Invalid("search <query>".into()))?;
            for hit in sources::gamesearch(&config.gamesearch_base, query, 20)? {
                println!("{:<12} {}", hit.slug, hit.title);
            }
            Ok(())
        }
        "download" => {
            let slug = args
                .get(1)
                .ok_or_else(|| Error::Invalid("download <slug>".into()))?;
            let detail = sources::gamesearch_detail(&config.gamesearch_base, slug)?;
            let label = detail.title.clone();
            run_job(&label, move |ctx| {
                let path = store::download_hit(&detail, &config, ctx)?;
                println!("downloaded to {}", path.display());
                Ok(())
            })
        }
        "homebrew" => {
            let refresh = has_flag(args, "--refresh");
            for entry in homebrew::catalog(refresh) {
                println!(
                    "{:<16} {:<22} {:<10} {}",
                    entry.id,
                    entry.name,
                    entry.latest_tag.unwrap_or_else(|| "-".into()),
                    entry.repo
                );
            }
            Ok(())
        }
        "homebrew-install" => {
            let id = args
                .get(1)
                .ok_or_else(|| Error::Invalid("homebrew-install <id>".into()))?;
            let host = config.require_active()?;
            let entry = homebrew::catalog(false)
                .into_iter()
                .find(|e| &e.id == id)
                .ok_or_else(|| Error::NotFound(format!("no homebrew {id}; refresh the catalog")))?;
            let label = entry.name.clone();
            run_job(&label, move |ctx| {
                let report = homebrew::install(&host, &config, &entry, ctx)?;
                println!("{}", report.message);
                Ok(())
            })
        }
        "pad-status" => {
            let status = pad::status(&config.require_active()?);
            println!(
                "service={} protocol={:?} pads={:?} version={:?}",
                if status.ok { "up" } else { "down" },
                status.protocol,
                status.pads,
                status.version
            );
            if !status.message.is_empty() {
                println!("{}", status.message);
            }
            Ok(())
        }
        other => Err(Error::Invalid(format!(
            "unknown command '{other}'; run godsend-ps4 help"
        ))),
    }
}

fn run_job<F>(label: &str, work: F) -> Result<()>
where
    F: FnOnce(&godsend_core::transfer::JobContext) -> Result<()> + Send + 'static,
{
    let manager = JobManager::new();
    let id = manager.spawn("cli", label, |_| {}, work);
    let mut last_pct = 0u64;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let Some(job) = manager.get(id) else { continue };
        let pct = if job.total_bytes > 0 {
            job.done_bytes * 100 / job.total_bytes
        } else {
            0
        };
        if pct >= last_pct + 10 {
            last_pct = pct;
            eprintln!("{label}: {pct}%");
        }
        match job.state {
            godsend_core::JobState::Running => {}
            godsend_core::JobState::Done => return Ok(()),
            godsend_core::JobState::Failed => {
                return Err(Error::Other(job.message));
            }
            godsend_core::JobState::Cancelled => {
                return Err(Error::Cancelled(job.id));
            }
        }
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|a| a == flag)?;
    args.get(index + 1).cloned()
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn auto_subnet() -> String {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(socket) = socket {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                let ip = addr.ip().to_string();
                let parts: Vec<&str> = ip.split('.').collect();
                if parts.len() == 4 {
                    return format!("{}.{}.{}", parts[0], parts[1], parts[2]);
                }
            }
        }
    }
    "192.168.1".into()
}
