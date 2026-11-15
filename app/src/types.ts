// Types shared with the Rust core (keep in sync with core/src serializers).

export interface Host {
  id: string;
  name: string;
  addr: string;
  ftp_port: number;
  ftp_user: string;
  ftp_pass: string;
  rpi_port: number;
  pad_port: number;
  binloader_port: number;
}

export interface Config {
  hosts: Host[];
  active_host: string | null;
  download_dir: string;
  rd_api_key: string;
  tb_api_key: string;
  gamesearch_base: string;
  covers_base: string;
  aria2_path: string;
  split_bytes: number;
  auto_install_pkg: boolean;
  mcp_port: number;
}

export interface DeviceStatus {
  host_id: string;
  name: string;
  addr: string;
  ftp_ok: boolean;
  ftp_ms: number;
  greeting: string | null;
  rpi_ok: boolean;
  rpi_ms: number;
  pad_ok: boolean;
  pad_ms: number;
  titles: number;
  packages: number;
  message: string;
}

export interface PadStatus {
  ok: boolean;
  version: string | null;
  protocol: number | null;
  pads: number | null;
  message: string;
}

export interface Check {
  name: string;
  ok: boolean;
  detail: string;
}

export type JobState = "running" | "done" | "failed" | "cancelled";

export interface Job {
  id: number;
  kind: string;
  label: string;
  state: JobState;
  done_bytes: number;
  total_bytes: number;
  message: string;
  started_at: number;
  finished_at: number | null;
}

export interface Snapshot {
  hosts: Host[];
  active: string | null;
  config: Config;
  doctor: Check[];
  statuses: DeviceStatus[];
  pad: PadStatus | null;
  jobs: Job[];
}

export interface GameEntry {
  id: string;
  title: string;
  title_id: string | null;
  content_id: string | null;
  system: string;
  kind: string;
  category: string;
  path: string;
  size_bytes: number;
  has_icon: boolean;
  icon_path: string | null;
  version: string | null;
  device: string;
}

export interface LibraryView {
  games: GameEntry[];
  packages: GameEntry[];
  roots: string[];
}

export interface SaveEntry {
  title_id: string;
  title: string | null;
  path: string;
  user: string;
  files: number;
  size_bytes: number;
}

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  repo: string;
  asset: string;
  kind: string;
  note: string | null;
  latest_tag: string | null;
  asset_url: string | null;
  published_at: string | null;
  size_bytes: number;
}

export interface GameLink {
  id: string;
  host: string;
  url: string;
  linkType: string;
  supportedBy: string[];
}

export interface GameHit {
  id: string;
  slug: string;
  title: string;
  platform: string;
  source: string;
  linkCount: number;
  rawgCover: string | null;
  cover: string | null;
  rating: number | null;
  released: string | null;
  links?: GameLink[];
}

export interface IaItem {
  id: string;
  title: string;
  files_count: number;
  size_bytes: number;
}

export interface IaFile {
  name: string;
  size_bytes: number;
  url: string;
}

export interface FmEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size_bytes: number;
  kind: string;
}

export interface ConvertInfo {
  path: string;
  name: string;
  size_bytes: number;
  kind: string;
  system: string;
  title_id: string | null;
  parts: number;
}

export const PAGE_TITLES: Record<string, string> = {
  home: "Console",
  devices: "Devices",
  library: "Library",
  controller: "Controller",
  store: "Game Index",
  downloads: "Downloads",
  files: "Files",
  convert: "Convert",
  homebrew: "Homebrew",
  saves: "Saves",
  settings: "Settings",
};
