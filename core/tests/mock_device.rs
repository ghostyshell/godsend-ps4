//! End-to-end test against a fake PS4: a GoldHEN-style FTP server, a remote
//! package installer on 12800 and a virtual pad service on 4264. Drives the
//! scan, icon fetch, upload/download, install (including the console pulling
//! the package back over HTTP) and the WebRTC offer exchange exactly as the app
//! does against hardware.

use godsend_core::config::{Config, Host};
use godsend_core::convert;
use godsend_core::ftp;
use godsend_core::install;
use godsend_core::library;
use godsend_core::pad;
use godsend_core::pkg;
use godsend_core::transfer::JobManager;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

thread_local! {
    static PASSIVE: std::cell::RefCell<Option<TcpListener>> = const { std::cell::RefCell::new(None) };
}

struct FakeFtp {
    port: u16,
}

impl FakeFtp {
    fn start(root: PathBuf) -> FakeFtp {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ftp");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let root = root.clone();
                std::thread::spawn(move || serve_control(stream, root));
            }
        });
        FakeFtp { port }
    }

    fn host(&self, rpi_port: u16, pad_port: u16) -> Host {
        let mut host = Host::new("test ps4", "127.0.0.1");
        host.ftp_port = self.port;
        host.rpi_port = rpi_port;
        host.pad_port = pad_port;
        host
    }
}

fn map(root: &Path, remote: &str) -> PathBuf {
    root.join(remote.trim_start_matches('/'))
}

fn serve_control(stream: TcpStream, root: PathBuf) {
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut cwd = "/".to_string();
    fn reply(writer: &mut TcpStream, text: &str) {
        let _ = writer.write_all(format!("{text}\r\n").as_bytes());
    }
    reply(&mut writer, "220 GoldHEN FTP server ready");
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        let (cmd, arg) = match line.split_once(' ') {
            Some((c, a)) => (c.to_uppercase(), a.trim().to_string()),
            None => (line.to_uppercase(), String::new()),
        };
        match cmd.as_str() {
            "USER" => reply(&mut writer, "331 password required"),
            "PASS" => reply(&mut writer, "230 logged in"),
            "TYPE" | "NOOP" | "OPTS" | "MODE" | "STRU" | "SITE" | "ABOR" => {
                reply(&mut writer, "200 ok")
            }
            "SYST" => reply(&mut writer, "215 UNIX Type: L8"),
            "FEAT" => {
                let _ = writer.write_all(b"211-Features:\r\n UTF8\r\n211 End\r\n");
            }
            "PWD" => reply(&mut writer, &format!("257 \"{cwd}\"")),
            "CWD" => {
                let target = if arg.starts_with('/') {
                    arg.clone()
                } else {
                    format!("{}/{}", cwd.trim_end_matches('/'), arg)
                };
                if map(&root, &target).is_dir() {
                    cwd = target;
                    reply(&mut writer, "250 ok");
                } else {
                    reply(&mut writer, "550 no such directory");
                }
            }
            "CDUP" => {
                if let Some((parent, _)) = cwd.trim_end_matches('/').rsplit_once('/') {
                    cwd = if parent.is_empty() {
                        "/".into()
                    } else {
                        parent.into()
                    };
                }
                reply(&mut writer, "250 ok");
            }
            "PASV" | "EPSV" => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = listener.local_addr().unwrap().port();
                if cmd == "EPSV" {
                    reply(
                        &mut writer,
                        &format!("229 Entering Extended Passive Mode (|||{port}|)"),
                    );
                } else {
                    reply(
                        &mut writer,
                        &format!(
                            "227 Entering Passive Mode (127,0,0,1,{},{})",
                            port / 256,
                            port % 256
                        ),
                    );
                }
                PASSIVE.with(|slot| *slot.borrow_mut() = Some(listener));
            }
            "MLSD" => reply(&mut writer, "502 MLSD not supported"),
            "LIST" => {
                let target = if arg.is_empty() || arg.starts_with('-') {
                    cwd.clone()
                } else {
                    arg.clone()
                };
                let path = map(&root, &target);
                reply(&mut writer, "150 here it comes");
                if let Some(listener) = PASSIVE.with(|slot| slot.borrow_mut().take()) {
                    if let Ok((mut data, _)) = listener.accept() {
                        if let Ok(entries) = std::fs::read_dir(&path) {
                            for entry in entries.flatten() {
                                let meta = entry.metadata().unwrap();
                                let name = entry.file_name().to_string_lossy().to_string();
                                let perm = if meta.is_dir() {
                                    "drwxr-xr-x"
                                } else {
                                    "-rw-r--r--"
                                };
                                let _ = data.write_all(
                                    format!(
                                        "{perm} 1 root root {} Jan 01 12:00 {name}\r\n",
                                        meta.len()
                                    )
                                    .as_bytes(),
                                );
                            }
                        }
                        let _ = data.flush();
                    }
                }
                reply(&mut writer, "226 transfer complete");
            }
            "SIZE" => {
                let path = map(&root, &arg);
                match std::fs::metadata(&path) {
                    Ok(meta) => reply(&mut writer, &format!("213 {}", meta.len())),
                    Err(_) => reply(&mut writer, "550 no such file"),
                }
            }
            "RETR" => {
                let path = map(&root, &arg);
                reply(&mut writer, "150 sending");
                if let Some(listener) = PASSIVE.with(|slot| slot.borrow_mut().take()) {
                    if let Ok((mut data, _)) = listener.accept() {
                        if let Ok(mut file) = std::fs::File::open(&path) {
                            let _ = std::io::copy(&mut file, &mut data);
                        }
                        let _ = data.flush();
                    }
                }
                reply(&mut writer, "226 transfer complete");
            }
            "STOR" => {
                let path = map(&root, &arg);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                reply(&mut writer, "150 receiving");
                if let Some(listener) = PASSIVE.with(|slot| slot.borrow_mut().take()) {
                    if let Ok((mut data, _)) = listener.accept() {
                        let mut file = std::fs::File::create(&path).unwrap();
                        let _ = std::io::copy(&mut data, &mut file);
                    }
                }
                reply(&mut writer, "226 transfer complete");
            }
            "MKD" => {
                let _ = std::fs::create_dir_all(map(&root, &arg));
                reply(&mut writer, &format!("257 \"{arg}\" created"));
            }
            "DELE" => {
                let _ = std::fs::remove_file(map(&root, &arg));
                reply(&mut writer, "250 deleted");
            }
            "RMD" => {
                let _ = std::fs::remove_dir_all(map(&root, &arg));
                reply(&mut writer, "250 removed");
            }
            "QUIT" => {
                reply(&mut writer, "221 bye");
                return;
            }
            _ => reply(&mut writer, "502 not implemented"),
        }
    }
}

struct FakeHttp {
    port: u16,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    /// Bytes the fake console pulled from the app during an install.
    pulled: Arc<Mutex<u64>>,
}

/// A tiny HTTP server. `routes` maps a path to its reply. `POST /api/install`
/// makes the fake console behave like the real one: it fetches the package URL
/// from the request body, which is what completes the app's install job.
fn fake_http(routes: Vec<(&'static str, String)>) -> FakeHttp {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let pulled = Arc::new(Mutex::new(0u64));
    let seen = requests.clone();
    let pulled_copy = pulled.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let seen = seen.clone();
            let pulled = pulled_copy.clone();
            let routes = routes.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let mut parts = head.lines().next().unwrap_or("").split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();
                let mut length = 0usize;
                for line in head.lines() {
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let body_text = String::from_utf8_lossy(&body).to_string();
                seen.lock().unwrap().push((
                    format!("{method} {path}"),
                    format!("{head}\n{body_text}"),
                ));

                let reply = routes
                    .iter()
                    .find(|(route, _)| *route == path)
                    .map(|(_, text)| text.clone())
                    .unwrap_or_else(|| "{}".to_string());
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                        reply.len()
                    )
                    .as_bytes(),
                );

                // Play the console: pull the package the app just announced.
                if method == "POST" && path == "/api/install" {
                    if let Some(url) = first_package_url(&body_text) {
                        if let Ok(bytes) = fetch(&url) {
                            *pulled.lock().unwrap() += bytes;
                        }
                    }
                }
            });
        }
    });
    FakeHttp {
        port,
        requests,
        pulled,
    }
}

/// `{"type":"direct","packages":["http://host:port/name.pkg"]}` -> the URL.
fn first_package_url(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("packages")?
        .as_array()?
        .first()?
        .as_str()
        .map(|s| s.to_string())
}

/// GET a URL and return the body length.
fn fetch(url: &str) -> std::io::Result<u64> {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut stream = TcpStream::connect(authority)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    stream.write_all(
        format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(0);
    Ok((response.len() - split) as u64)
}

fn pkg_bytes() -> Vec<u8> {
    let mut head = vec![0u8; 4096];
    head[0..4].copy_from_slice(&pkg::MAGIC);
    head[4..8].copy_from_slice(&1u32.to_be_bytes());
    head[0x30..0x30 + 36].copy_from_slice(b"UP0001-CUSA07410_00-0000000000000000");
    head
}

fn fixture_tree() -> PathBuf {
    let root = std::env::temp_dir().join(format!("godsend-ps4-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    let meta = root.join("user/appmeta/CUSA07410");
    std::fs::create_dir_all(&meta).unwrap();
    std::fs::write(
        meta.join("param.json"),
        br#"{"titleId":"CUSA07410","titleName":"God of War","contentId":"UP9000-CUSA07410_00-0000000000000000","version":"01.00","category":"gd"}"#,
    )
    .unwrap();
    std::fs::write(meta.join("icon0.png"), b"\x89PNG\r\n\x1a\nfake").unwrap();

    let external = root.join("user/appmeta/external/CUSA00002");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(
        external.join("param.json"),
        br#"{"titleId":"CUSA00002","titleName":"USB Homebrew","contentId":"IV0000-HBOM00001_00-ABC"}"#,
    )
    .unwrap();

    let patch = root.join("user/appmeta/CUSA07410-p");
    std::fs::create_dir_all(&patch).unwrap();
    std::fs::write(
        patch.join("param.json"),
        br#"{"titleId":"CUSA07410","titleName":"God of War","category":"gp"}"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("user/pkg")).unwrap();
    std::fs::write(root.join("user/pkg/update.pkg"), pkg_bytes()).unwrap();

    let save = root.join("user/home/10000000/savedata/CUSA07410/sce_sys");
    std::fs::create_dir_all(&save).unwrap();
    std::fs::write(save.join("sdslot.dat"), vec![7u8; 64]).unwrap();
    root
}

#[test]
fn drives_a_fake_console_end_to_end() {
    let root = fixture_tree();
    let ftp_server = FakeFtp::start(root.clone());
    let rpi = fake_http(vec![("/api/status", "{}".into())]);
    let pad_server = fake_http(vec![
        (
            "/api/status",
            r#"{"version":"1.3.1","protocol":3,"pads":4,"motion":true,"input":true}"#.into(),
        ),
        ("/rtc", "v=0\r\na=ice-lite\r\n".into()),
    ]);
    let host = ftp_server.host(rpi.port, pad_server.port);

    // 1. The device card sees a live console.
    let status = godsend_core::device::probe(&host);
    assert!(status.ftp_ok, "message: {}", status.message);
    assert!(status.greeting.unwrap().contains("GoldHEN"));
    assert!(status.rpi_ok);
    assert!(status.pad_ok);
    assert!(status.titles >= 2, "appmeta dirs: {}", status.titles);

    // 2. Library scan: installed titles, the USB title, the package, the save.
    let view = library::scan(&host).expect("library scan");
    let titles: Vec<&str> = view.games.iter().map(|g| g.title.as_str()).collect();
    assert!(titles.contains(&"God of War"), "titles: {titles:?}");
    assert!(titles.contains(&"USB Homebrew"), "titles: {titles:?}");
    let gow = view.games.iter().find(|g| g.title == "God of War").unwrap();
    assert_eq!(gow.title_id.as_deref(), Some("CUSA07410"));
    assert_eq!(gow.version.as_deref(), Some("01.00"));
    assert_eq!(view.packages.len(), 1);
    assert_eq!(view.packages[0].title_id.as_deref(), Some("CUSA07410"));

    let saves = godsend_core::saves::list(&host).expect("saves");
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].title.as_deref(), Some("God of War"));

    // 3. Icon fetch over FTP.
    let icon = library::icon_bytes(&host, "/user/appmeta/CUSA07410/icon0.png").expect("icon");
    assert!(icon.starts_with(b"\x89PNG"));

    // 4. Upload a file and download it back.
    let local = root.join("upload.bin");
    std::fs::write(&local, b"hello ps4").unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let host_upload = host.clone();
    let local_upload = local.clone();
    JobManager::new().spawn(
        "upload",
        "u",
        |_| {},
        move |ctx| {
            let mut conn = ftp::connect(&host_upload)?;
            ftp::upload(&mut conn, &local_upload, "/user/pkg/upload.bin", ctx)?;
            let _ = conn.quit();
            tx.send(()).unwrap();
            Ok(())
        },
    );
    rx.recv_timeout(Duration::from_secs(10))
        .expect("upload finished");
    assert_eq!(
        std::fs::read(root.join("user/pkg/upload.bin")).unwrap(),
        b"hello ps4"
    );

    let downloaded = root.join("downloaded.bin");
    let (tx2, rx2) = std::sync::mpsc::channel();
    let host_download = host.clone();
    JobManager::new().spawn(
        "download",
        "d",
        |_| {},
        move |ctx| {
            let mut conn = ftp::connect(&host_download)?;
            ftp::download(&mut conn, "/user/pkg/upload.bin", &downloaded, ctx)?;
            let _ = conn.quit();
            tx2.send(()).unwrap();
            Ok(())
        },
    );
    rx2.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        std::fs::read(root.join("downloaded.bin")).unwrap(),
        b"hello ps4"
    );

    // 5. Install a package: the installer is asked, then the console pulls the
    //    bytes from the app's one-shot HTTP server.
    let package = root.join("game.pkg");
    std::fs::write(&package, pkg_bytes()).unwrap();
    let mut config = Config::default();
    config.auto_install_pkg = true;
    let (tx3, rx3) = std::sync::mpsc::channel();
    let host_install = host.clone();
    let package_install = package.clone();
    let manager = JobManager::new();
    manager.spawn(
        "install",
        "game.pkg",
        |_| {},
        move |ctx| {
            let report = install::install_local(&host_install, &package_install, &config, ctx)?;
            tx3.send(report.install_requested).unwrap();
            Ok(())
        },
    );
    assert!(
        rx3.recv_timeout(Duration::from_secs(20))
            .expect("install job finished"),
        "the console's installer must be used when it answers"
    );
    let calls = rpi.requests.lock().unwrap().clone();
    let install_call = calls
        .iter()
        .find(|(line, _)| line == "POST /api/install")
        .expect("install request");
    assert!(
        install_call.1.contains("http://127.0.0.1:"),
        "the install body must carry the package URL: {}",
        install_call.1
    );
    let expected = pkg_bytes().len() as u64;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut pulled = 0u64;
    while std::time::Instant::now() < deadline {
        pulled = *rpi.pulled.lock().unwrap();
        if pulled >= expected {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        pulled >= expected,
        "the console must pull the whole package, got {pulled} bytes"
    );

    // 6. Virtual pad: status, then an offer exchange with no Origin header.
    let pad_status = pad::status(&host);
    assert!(pad_status.ok);
    assert_eq!(pad_status.protocol, Some(3));
    assert_eq!(pad_status.pads, Some(4));
    let answer = pad::exchange(&host, "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\n").expect("exchange");
    assert!(answer.contains("ice-lite"));
    let pad_calls = pad_server.requests.lock().unwrap().clone();
    let offer = pad_calls
        .iter()
        .find(|(line, _)| line == "POST /rtc")
        .expect("offer");
    assert!(
        !offer.1.to_lowercase().contains("origin:"),
        "the payload refuses a foreign Origin, so none may be sent: {}",
        offer.1
    );

    // 7. Convert inspection sees the package.
    let info = convert::inspect(&package).expect("inspect");
    assert_eq!(info.kind, "pkg");
    assert_eq!(info.title_id.as_deref(), Some("CUSA07410"));

    let _ = std::fs::remove_dir_all(&root);
}
