# AI guidelines for godsend-ps4

Read `AGENTS.md` first for the architecture and invariants, and `DESIGN.md`
before touching any UI code.

## Stack

Rust workspace (`core` engine, `cli`, `mcp`, `app/src-tauri` Tauri 2 shell)
plus a React 18 + TypeScript + Vite frontend with one hand-rolled stylesheet.
No CSS framework; design tokens live on `:root` in `app/src/styles.css` and
raw hex in components is banned.

## Rules

1. Keep `core/` framework-agnostic: no tauri, no UI types, no stdio. The CLI,
   the MCP server and the desktop app are thin layers over the engine.
2. Frontend types in `app/src/types.ts` must stay in sync with the serde
   shapes in core; the mock bridge (`app/src/mock.ts`) must mirror real
   fixture shapes so `?mock=1` stays honest.
3. Long operations are jobs in `godsend_core::transfer::JobManager` with
   progress and cancellation; never block the UI thread with I/O.
4. Destructive console operations require explicit confirmation in the UI and
   must path-escape-guard anything joined from user input.
5. Copy follows the no-em-dashes rule. Every product change bumps the version
   and writes a `CHANGELOG.md` entry in the same commit:
   `scripts/bump-version.sh x.y.z "Fixed: ..."`. No Unreleased section.
6. Design: follow DESIGN.md tokens and motion rules; respect the
   reduced-motion kill switch; keep state chips dot plus word.
7. Icons come from `assets/logo-source.png`. Regenerate with
   `npx --prefix app tauri icon assets/logo-source.png`; keep
   `app/src/assets/logo.png` the same mark.
8. Tests: `cargo test --workspace` covers parsers, `param.json` fallbacks,
   the PKG header scan, the fake-console end-to-end flow and MCP;
   `npm --prefix app run test:ui` covers every page through the mock bridge.
   Both must pass before a commit.

## PS4 domain notes

- Everything is network based: FTP on 2121 (GoldHEN), the remote package
  installer on 12800, the bin loader on 9090, the Control4Free pad on 4264.
  There is no mounted volume and no system-info page.
- Remote paths are absolute: `/user/appmeta`, `/user/pkg`, `/user/home`,
  `/mnt/usb0`, `/mnt/ext0`.
- Installed titles are identified by `param.json` under appmeta. Key names
  vary by GoldHEN build; always fall back to the content id.
- A PS4 installs PKG files, not folders. Never copy a tree into `/user/app`.
- Installed titles are removed from the console's own storage menu; deleting
  folders over FTP leaves a ghost entry, so the app refuses.
- The installer takes a URL, not an upload: serve the file on the interface
  that reaches the console and post `{"type":"direct","packages":[url]}`.
- The pad payload refuses a foreign `Origin`, so the SDP exchange runs in
  Rust; the webview owns the peer connection and the two negotiated data
  channels (ids 0 and 1).
- FAT32 splits are `name.0`, `name.1` at 4095 MiB by default.
