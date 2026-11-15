// Mock bridge: installs window.__TAURI_INTERNALS__ with PS4 fixture data so
// the UI runs in a plain browser (?mock=1). Never imported by the app.

type Handler = (payload: unknown) => void;

const listeners = new Map<number, { event: string; handler: Handler }>();
let nextCallbackId = 1;

const MB = 1024 * 1024;
const GB = 1024 * 1024 * 1024;

const host = {
  id: "192.168.1.60:2121",
  name: "Living room PS4",
  addr: "192.168.1.60",
  ftp_port: 2121,
  ftp_user: "anonymous",
  ftp_pass: "",
  rpi_port: 12800,
  pad_port: 4264,
  binloader_port: 9090,
};

let config = {
  hosts: [host],
  active_host: host.id,
  download_dir: "/Users/demo/Downloads/godsend-ps4",
  rd_api_key: "",
  tb_api_key: "",
  gamesearch_base: "https://game-debrid.link/api",
  covers_base: "https://covers.debrid.in",
  aria2_path: "",
  split_bytes: 4095 * MB,
  auto_install_pkg: true,
  mcp_port: 8744,
};

const status = {
  host_id: host.id,
  name: host.name,
  addr: host.addr,
  ftp_ok: true,
  ftp_ms: 14,
  greeting: "GoldHEN FTP server ready",
  rpi_ok: true,
  rpi_ms: 21,
  pad_ok: true,
  pad_ms: 19,
  titles: 7,
  packages: 2,
  message: "",
};

const pad = {
  ok: true,
  version: "1.3.1",
  protocol: 3,
  pads: 4,
  message: "",
};

const games = [
  {
    id: "app:/user/appmeta/CUSA07410",
    title: "God of War",
    title_id: "CUSA07410",
    content_id: "UP9000-CUSA07410_00-0000000000000000",
    system: "ps4",
    kind: "app",
    category: "gd",
    path: "/user/appmeta/CUSA07410",
    size_bytes: 39 * GB,
    has_icon: true,
    icon_path: "/user/appmeta/CUSA07410/icon0.png",
    version: "01.33",
    device: "internal",
  },
  {
    id: "app:/user/appmeta/CUSA10209",
    title: "Bloodborne",
    title_id: "CUSA10209",
    content_id: "UP9000-CUSA10209_00-0000000000000000",
    system: "ps4",
    kind: "app",
    category: "gd",
    path: "/user/appmeta/CUSA10209",
    size_bytes: 32 * GB,
    has_icon: true,
    icon_path: "/user/appmeta/CUSA10209/icon0.png",
    version: "01.09",
    device: "internal",
  },
  {
    id: "app:/user/appmeta/CUSA00203",
    title: "Astro Bot Rescue Mission",
    title_id: "CUSA00203",
    content_id: "UP9000-CUSA00203_00-0000000000000000",
    system: "ps4",
    kind: "app",
    category: "gd",
    path: "/user/appmeta/CUSA00203",
    size_bytes: 8 * GB,
    has_icon: true,
    icon_path: "/user/appmeta/CUSA00203/icon0.png",
    version: "01.02",
    device: "internal",
  },
  {
    id: "app:/user/appmeta/external/HBOM00001",
    title: "Control4Free",
    title_id: "HBOM00001",
    content_id: "IV0000-HBOM00001_00-0000000000000000",
    system: "ps4",
    kind: "homebrew",
    category: "homebrew",
    path: "/user/appmeta/external/HBOM00001",
    size_bytes: 7 * MB,
    has_icon: true,
    icon_path: "/user/appmeta/external/HBOM00001/icon0.png",
    version: "01.03",
    device: "external",
  },
];

const packages = [
  {
    id: "pkg:/user/pkg/APOLLO.pkg",
    title: "APOLLO",
    title_id: "APOL00004",
    content_id: "IV0000-APOL00004_00-APOLLO0000000PS4",
    system: "ps4",
    kind: "pkg",
    category: "package",
    path: "/user/pkg/APOLLO.pkg",
    size_bytes: 22 * MB,
    has_icon: false,
    icon_path: null,
    version: null,
    device: "internal",
  },
  {
    id: "pkg:/user/pkg/update.pkg",
    title: "update",
    title_id: null,
    content_id: null,
    system: "ps4",
    kind: "pkg",
    category: "package",
    path: "/user/pkg/update.pkg",
    size_bytes: 1.2 * GB,
    has_icon: false,
    icon_path: null,
    version: null,
    device: "internal",
  },
];

const saves = [
  {
    title_id: "CUSA07410",
    title: "God of War",
    path: "/user/home/10000000/savedata/CUSA07410",
    user: "10000000",
    files: 3,
    size_bytes: 2.4 * MB,
  },
  {
    title_id: "CUSA10209",
    title: "Bloodborne",
    path: "/user/home/10000000/savedata/CUSA10209",
    user: "10000000",
    files: 2,
    size_bytes: 1.1 * MB,
  },
];

const catalog = [
  {
    id: "control4free",
    name: "Control4Free",
    description:
      "Virtual DualShock 4 over the network, up to four controllers. The Controller page talks to it on port 4264.",
    category: "pad",
    repo: "MoHadiShibli/Control4Free",
    asset: "Control4Free-",
    kind: "pkg",
    note: "Install this to use the Controller page.",
    latest_tag: "1.3.1",
    asset_url: "https://github.com/MoHadiShibli/Control4Free/releases/download/v1.3.1/Control4Free-1.3.1.pkg",
    published_at: "2026-10-02T10:00:00Z",
    size_bytes: 7 * MB,
  },
  {
    id: "apollo",
    name: "Apollo Save Tool",
    description:
      "Save data manager: back up, restore and resign saves, and download saves from the community database.",
    category: "save",
    repo: "bucanero/apollo-ps4",
    asset: "APOLLO",
    kind: "pkg",
    note: null,
    latest_tag: "v2.4.0",
    asset_url: "https://github.com/bucanero/apollo-ps4/releases/download/v2.4.0/APOLLO.pkg",
    published_at: "2026-09-18T10:00:00Z",
    size_bytes: 22 * MB,
  },
];

const doctor = [
  { name: "Console configured", ok: true, detail: "Living room PS4 at 192.168.1.60" },
  { name: "FTP reachable", ok: true, detail: "14 ms - GoldHEN FTP server ready" },
  { name: "Package installer", ok: true, detail: "port 12800 answers" },
  { name: "Virtual pad", ok: true, detail: "port 4264 answers" },
  { name: "Library", ok: true, detail: "7 titles installed, 2 packages waiting" },
  { name: "Download folder", ok: true, detail: "/Users/demo/Downloads/godsend-ps4" },
  { name: "aria2c", ok: false, detail: "optional: downloads fall back to a single connection" },
  { name: "Debrid", ok: false, detail: "optional: keys unlock debrid download links" },
  { name: "MCP server", ok: true, detail: "http://127.0.0.1:8744/mcp, started with the app" },
];

const jobs = [
  {
    id: 1,
    kind: "install",
    label: "Control4Free-1.3.1.pkg",
    state: "done",
    done_bytes: 7 * MB,
    total_bytes: 7 * MB,
    message: "package delivered; the console installs it in the background",
    started_at: 1,
    finished_at: 2,
  },
];

const fmFiles = [
  { name: "appmeta", path: "/user/appmeta", is_dir: true, size_bytes: 0, kind: "folder" },
  { name: "pkg", path: "/user/pkg", is_dir: true, size_bytes: 0, kind: "folder" },
  { name: "homebrew", path: "/user/homebrew", is_dir: true, size_bytes: 0, kind: "folder" },
];

async function invoke(cmd: string, args: Record<string, any> = {}): Promise<unknown> {
  switch (cmd) {
    case "snapshot":
      return {
        hosts: config.hosts,
        active: config.active_host,
        config,
        doctor,
        statuses: [status],
        pad,
        jobs,
      };
    case "probe_host":
    case "probe_all":
      return cmd === "probe_all" ? [status] : status;
    case "discover":
      return [host];
    case "host_add": {
      const next = args.host;
      config = {
        ...config,
        hosts: [...config.hosts.filter((h) => h.id !== next.id), next],
      };
      return config;
    }
    case "host_remove":
      config = { ...config, hosts: config.hosts.filter((h) => h.id !== args.id) };
      return config;
    case "host_set_active":
      config = { ...config, active_host: args.id };
      return config;
    case "library_scan":
      return { games, packages, roots: ["/user/appmeta", "/user/pkg"] };
    case "icon":
      return null;
    case "library_delete":
    case "saves_delete":
      return null;
    case "send_payload":
      return 1;
    case "pad_status":
      return pad;
    case "pad_offer":
      return "v=0\r\na=ice-lite\r\n";
    case "config_set":
      config = { ...config, ...args.patch };
      return config;
    case "config_get":
      return config;
    case "homebrew_catalog":
      return catalog;
    case "homebrew_install":
    case "store_download":
    case "download_push":
    case "install_path":
    case "fm_upload":
    case "fm_download":
    case "convert_run":
    case "saves_backup":
    case "saves_restore":
      return 1;
    case "store_search":
      return args.source === "ia"
        ? {
            kind: "ia",
            items: [
              { id: "ps4-homebrew", title: "PS4 homebrew collection", files_count: 12, size_bytes: 3 * GB },
            ],
          }
        : { kind: "gamesearch", games: [] };
    case "store_files":
      return [{ name: "game.pkg", size_bytes: 12 * GB, url: "https://archive.org/game.pkg" }];
    case "downloads_list":
      return {
        dir: config.download_dir,
        files: [
          { name: "Control4Free-1.3.1.pkg", path: "x", is_dir: false, size_bytes: 7 * MB, kind: "pkg" },
        ],
      };
    case "download_open":
      return "opened";
    case "download_delete":
      return null;
    case "fm_local":
    case "fm_remote":
      return fmFiles;
    case "fm_mkdir":
      return "/tmp/new";
    case "fm_mkdir_remote":
    case "fm_delete":
      return null;
    case "convert_inspect": {
      const path = String(args.path ?? "");
      const name = path.split("/").pop() ?? "file";
      const kind = name.endsWith(".pkg")
        ? "pkg"
        : name.endsWith(".0")
        ? "split"
        : name.endsWith(".elf")
        ? "payload"
        : "other";
      return {
        path,
        name,
        size_bytes: 12 * GB,
        kind,
        system: kind === "pkg" ? "pkg" : "unknown",
        title_id: kind === "pkg" ? "CUSA07410" : null,
        parts: kind === "split" ? 3 : 0,
      };
    }
    case "saves_list":
      return saves;
    case "doctor_run":
      return doctor;
    case "jobs":
      return jobs;
    case "job_cancel":
      return true;
    case "jobs_clear":
      return null;
    case "reveal":
      return null;
    case "open_data_dir":
      return "/Users/demo/Library/Application Support/godsend-ps4";
    case "plugin:dialog|open":
      return "/Users/demo/game.pkg";
    case "plugin:dialog|save":
      return "/Users/demo/out.pkg";
    case "plugin:event|listen": {
      const id = nextCallbackId++;
      listeners.set(id, { event: args.event, handler: args.handler });
      return id;
    }
    case "plugin:event|unlisten":
      listeners.delete(args.eventId);
      return null;
    default:
      console.warn("mock: unhandled command", cmd, args);
      return null;
  }
}

if (new URLSearchParams(window.location.search).has("mock")) {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: Handler) => {
      const id = nextCallbackId++;
      listeners.set(id, { event: "", handler: cb });
      return id;
    },
  };
  // eslint-disable-next-line no-console
  console.info("GODsend PS4 mock bridge installed");
}

export {};
