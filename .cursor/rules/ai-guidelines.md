# AI guidelines for godsend-ps3

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
8. Tests: `cargo test --workspace` covers parsers, the ISO writer, decryption,
   the fake-console end-to-end flow and MCP; `npm --prefix app run test:ui`
   covers every page through the mock bridge. Both must pass before a commit.

## PS3 domain notes

- Detection is network based: FTP on port 21 plus webMAN MOD on port 80.
  There is no mounted volume.
- Remote paths are absolute and start with `/dev_`: `dev_hdd0` is internal,
  `dev_usb000` and friends are external.
- Games live in `dev_hdd0/game` (digital), `dev_hdd0/GAMES` (JB folders),
  `dev_hdd0/PS3ISO`, `PS2ISO`, `PSXISO`, `PSPISO`; packages in `pkg` and
  `packages`.
- webMAN commands are concatenated paths (`/install.ps3/dev_hdd0/...`,
  `/mount.ps3/dev_hdd0/...`) except its own query commands
  (`pad.ps3?cross`, `reboot.ps3?quick`).
- FAT32 splits are `name.0`, `name.1` at 4095 MiB by default.
- Redump decryption is AES-128-CBC per 2048-byte sector with a sector-index
  IV. Leave the region map zeroed after a successful run.
