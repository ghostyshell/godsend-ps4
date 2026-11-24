# Changelog

All notable changes to GODsend PS4 are recorded here, newest first.

## [Unreleased]

### Changed

- Downloads are hosted on Cloudflare R2 under versioned, immutable keys, so a
  link stays valid until the next version. The weekly GoFile/file.kiwi re-upload
  is gone: the scheduled job now only checks the live feed for a version bump,
  and each publish prunes R2 versions older than the previous one.

## [0.1.0] - 2026-11-15

First release.

- Console management over GoldHEN FTP: add, edit and remove consoles, scan the
  LAN on ports 2121 and 21, and probe FTP, the remote installer and the pad
  service.
- Library scan of `/user/appmeta`, `/user/appmeta/external`, `/user/pkg`,
  `/user/homebrew` and mounted USB volumes, with icons fetched lazily and
  cached by path hash. Patches and DLC stay out of the grid.
- Remote package install: the app serves a local PKG over HTTP and asks
  GoldHEN's installer (port 12800) to pull it, with resume support and a
  fallback to `/user/pkg`.
- Payload sending over the bin loader on port 9090.
- Virtual DualShock 4: the webview owns the WebRTC peer connection and streams
  the Control4Free input message at 60 Hz; the native side posts the offer
  because the payload refuses a foreign `Origin`. Buttons, triggers, both
  sticks, keyboard mapping and four controller slots.
- Homebrew catalog with the Control4Free package and Apollo Save Tool.
- File manager for both sides, with tree upload, download and FAT32 splitting.
- Conversions: PKG unpacking through `orbis-pkg-util`, split and merge.
- Saves: list, zip backup, restore and delete under
  `/user/home/<user>/savedata`.
- Game index and Internet Archive search with Real-Debrid and TorBox
  resolution and aria2 downloads.
- CLI (`godsend-ps4`) and MCP server (`godsend-ps4-mcp`) over the same engine.
- New chrome and blue mark as the app icon, matching the GODsend family.
