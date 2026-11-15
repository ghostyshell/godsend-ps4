# Features

GODsend PS4 manages a jailbroken PS4 running GoldHEN, over FTP, its remote
package installer, its bin loader and the Control4Free virtual pad.

## Consoles

- Multiple consoles, one active at a time.
- FTP on 2121 with the anonymous or `ftp` convention and automatic fallback.
- The FTP banner names the payload, so the device card shows what is serving.
- Remote installer detection on 12800 and pad service detection on 4264.
- Installed title and waiting package counts from `/user/appmeta` and
  `/user/pkg`.
- LAN discovery on ports 2121 and 21.

## Library

- Installed titles from `/user/appmeta`, including USB titles under
  `external`.
- Title name, id, content id and version from `param.json`, with fallbacks for
  every GoldHEN shape.
- Icons fetched over FTP and cached.
- Patches and DLC filtered out of the game grid.
- Packages waiting in `/user/pkg` and on mounted USB volumes.
- Loose homebrew files in `/user/homebrew`.

## Virtual controller

- On-screen DualShock 4: face buttons, d-pad, shoulders, analog triggers, both
  sticks, share, options, PS and touchpad click.
- Up to four controller slots.
- Keyboard mapping for every control.
- 60 Hz input over a WebRTC data channel, with the SDP exchange handled in
  Rust because the payload refuses a foreign `Origin`.

## Installs

- PKG streamed to GoldHEN's remote installer: nothing is copied to the
  console's storage twice.
- Resume support on the served file.
- Fallback upload to `/user/pkg` when no installer answers.
- Payloads sent over the bin loader on 9090.
- ZIP unpacking with package root detection and macOS junk pruning.

## Conversions

- PKG unpacking to a folder, with PFS decryption handled by `orbis-pkg-util`.
- Split for FAT32 and merge back.
- Package inspection: magic, type, title id, content id.

## Saves

- Saves listed under `/user/home/<user>/savedata/<TITLE_ID>`.
- Zip backup into the download folder, restore back to the console, delete
  with confirmation.

## Downloads

- Game index and Internet Archive search, with file lists.
- Homebrew catalog from GitHub releases (PKG or payload assets).
- aria2c with 16 connections when available, plain HTTP otherwise.
- Cancellable job queue with progress and a journal across restarts.

## Files

- Local and console panes.
- Upload, download, create folder, delete.
- FAT32 splitting on the way to a USB volume.

## MCP server

- Loopback HTTP endpoint that starts with the app, default
  `http://127.0.0.1:8744/mcp`.
- Tools for status, library, install, payload, files, conversions, saves,
  search, homebrew and the pad.
- Standalone stdio binary `godsend-ps4-mcp` for clients that spawn a server.

## Out of scope

- Building a PKG: a fake PKG carries Sony's package signature.
- Removing installed titles: the console's own storage menu owns that
  database, and deleting folders over FTP would leave a ghost entry.
- PKG decryption keys: not needed for fake PKGs, and not shipped.
