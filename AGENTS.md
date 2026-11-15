# GODsend PS4: project overview for agents

GODsend PS4 is a desktop workshop for a jailbroken PS4 running GoldHEN. It
speaks FTP for files, GoldHEN's remote package installer for installs, its bin
loader for payloads, and a WebRTC data channel to the Control4Free payload for
the virtual DualShock 4. It wraps all of that in the GODsend family shell
(Rust workspace, Tauri 2, React).

## Components

- **`core/` (`godsend-core`)**: framework-agnostic engine. No Tauri, no CLI
  parsing.
  - `config`: hosts (FTP, installer, pad and bin loader ports), active host,
    download folder, debrid keys, service bases, split size, install toggle,
    MCP port.
  - `ftp`: `suppaftp` wrapper. Tolerant listing (MLSD, Unix LIST, DOS LIST),
    download, upload, `upload_bytes`, `read_head`, `mkdir_all`, recursive
    remove. Paths are absolute (`/user/...`, `/mnt/usb0/...`).
  - `device`: host probing (FTP latency and banner, installer and pad
    reachability, appmeta and pkg counts) and LAN discovery on 2121 and 21.
  - `param`: `/user/appmeta/<TITLE_ID>/param.json` parsing. GoldHEN builds
    disagree on key names; every field tries the known spellings and falls
    back to the content id.
  - `library`: scan of `/user/appmeta`, `/user/appmeta/external`, `/user/pkg`,
    `/mnt/{usb0,usb1,ext0,ext1}` and `/user/homebrew`. Patches (`gp`) and DLC
    (`ac`) are not games and are skipped. Icons are fetched lazily.
  - `rpi`: remote package install. Serves the package on a one-shot HTTP
    listener, posts `{"type":"direct","packages":[url]}` to `/api/install`,
    then streams the file with range support.
  - `install`: classify a local path (PKG, payload, unpacked tree, ZIP),
    install it, send payloads over the bin loader (9090), upload trees.
  - `pkg`: PKG magic and content id scan for remote heads, plus extraction via
    `orbis-pkg-util` (PFS decryption included).
  - `pad`: Control4Free vocabulary. Button bit mask, the SDP exchange (native
    HTTP, no `Origin`), and status probing.
  - `saves`: save folders, zip backup, restore, delete.
  - `convert`, `split`: PKG extraction, FAT32 split and merge.
  - `homebrew`: GitHub release catalog with an asset fragment per entry, and
    the install flow (download, then install).
  - `sources`, `store`, `debrid`, `aria2`: game index and Internet Archive
    search, link selection, Real-Debrid and TorBox resolution, downloads.
  - `transfer`: JobManager with a progress hook, cancellation and a journal.
- **`cli/` (`godsend-ps4`)**: one command per feature area. Run with no
  arguments for the list.
- **`mcp/` (`godsend-ps4-mcp`)**: Model Context Protocol server. One tool
  handler over JSON-RPC, two transports: stdio for clients that spawn the
  binary, and loopback HTTP started by the desktop app on `mcp_port`.
- **`app/`**: Tauri 2 shell. `app/src-tauri/src/lib.rs` has the command layer;
  `app/src` is React 18 plus TypeScript with one hand-rolled `styles.css`
  (see DESIGN.md) and a mock bridge in `mock.ts` behind `?mock=1`.

## Invariants

- `core/` stays UI-free. It returns `Result<T>`; the Tauri error serializes as
  the Display string.
- A console is addressed over FTP; nothing assumes a mounted volume. Remote
  paths are absolute and start with `/user/` or `/mnt/`.
- FTP servers disagree about listings and the anonymous user. Try MLSD then
  LIST, and `anonymous` then `ftp`.
- The remote installer takes a URL, never an upload. The app's HTTP server
  binds `0.0.0.0` and advertises the interface address that reaches the
  console; do not advertise `127.0.0.1`.
- The pad payload refuses a request whose `Origin` is not its own or `null`.
  A webview always sends one, so the offer exchange runs in Rust; the webview
  only owns the peer connection and the data channels (ids 0 and 1,
  negotiated).
- Input is `{"method":"i","params":[pad,seq,buttons,lx,ly,rx,ry,l2,r2,
  fingers,...,historyCount,...]}`: sticks and triggers 0..255 with 128 centred.
  Send at 60 Hz; stale sequences are ignored, which repairs a lost release.
- Installed titles are removed from the PS4's own storage menu. Overwriting
  `/user/app` would leave an entry the console cannot launch, so the app
  refuses to delete them over FTP and says why.
- A PS4 installs PKG files, not folders. An unpacked tree is not installable;
  the app says so instead of copying it into `/user/app`.
- Destructive operations (file delete, save delete, payload send, install)
  need an explicit user confirmation in the UI.
- FTP is plaintext; never send credentials anywhere else and never log them.

## Building and testing

```sh
cargo test --workspace                  # unit tests plus the fake-console test
cargo check --workspace                 # zero warnings expected
npm --prefix app install
npm --prefix app run build              # tsc --noEmit plus vite build
npm --prefix app run dev:mock           # browser UI, no hardware
npm --prefix app run test:ui            # Playwright against the mock bridge
```

`core/tests/mock_device.rs` runs a GoldHEN-style FTP server, a fake remote
installer that pulls the package back over HTTP, and a fake pad service. It
drives scan, icons, upload/download, install, the SDP exchange and conversion
inspection. Extend it when a flow changes; it is the only test that exercises
the install loop end to end.

The UI dev server is on port 1436. Sibling apps use 1420, and Playwright's
`reuseExistingServer` will happily test the wrong app if the ports collide.

## Releases and copy

- Version lives in `Cargo.toml` (workspace), `app/package.json`,
  `app/src-tauri/tauri.conf.json`, `app/src/App.tsx` and the Settings about
  line. Use `scripts/bump-version.sh x.y.z "note"`; keep the newest CHANGELOG
  header at the top, no Unreleased section.
- Public-facing copy: no em dashes, no en dashes, ASCII hyphen only.
- Icons: `assets/logo-source.png` is the mark (the chrome and blue download
  emblem). Regenerate the set with
  `npx --prefix app tauri icon assets/logo-source.png` and copy the same file
  to `app/src/assets/logo.png`.

## Demo videos and guides

GODsend apps carry per-feature demo reels and a guides page, recorded from the
renderer's `?mock=1` bridge by `~/Code/demo-reels`. A new, changed or removed
feature updates its reel and its guides entry in the same change. The PS4
app's reels are not recorded yet; until they are, text guides live in
`docs/guides/`.
