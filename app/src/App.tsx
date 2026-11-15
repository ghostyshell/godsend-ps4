import { useCallback, useEffect, useMemo, useState } from "react";
import { api, listen } from "./api";
import type { Job, Snapshot } from "./types";
import { PAGE_TITLES } from "./types";
import { Chip } from "./ui";
import {
  House,
  MonitorSmartphone,
  Library as LibraryIcon,
  Gamepad2,
  Store as StoreIcon,
  Download as DownloadIcon,
  FolderOpen,
  ArrowLeftRight,
  Package as PackageIcon,
  Save as SaveIcon,
  Settings as SettingsIcon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import logo from "./assets/logo.png";
import Home from "./pages/Home";
import Devices from "./pages/Devices";
import Library from "./pages/Library";
import Controller from "./pages/Controller";
import GameStore from "./pages/GameStore";
import Downloads from "./pages/Downloads";
import Files from "./pages/Files";
import Convert from "./pages/Convert";
import Homebrew from "./pages/Homebrew";
import Saves from "./pages/Saves";
import Settings from "./pages/Settings";

type Page =
  | "home"
  | "devices"
  | "library"
  | "controller"
  | "store"
  | "downloads"
  | "files"
  | "convert"
  | "homebrew"
  | "saves"
  | "settings";

const NAV: { id: Page; label: string; icon: LucideIcon }[] = [
  { id: "home", label: "Console", icon: House },
  { id: "devices", label: "Devices", icon: MonitorSmartphone },
  { id: "library", label: "Library", icon: LibraryIcon },
  { id: "controller", label: "Controller", icon: Gamepad2 },
  { id: "store", label: "Game Index", icon: StoreIcon },
  { id: "downloads", label: "Downloads", icon: DownloadIcon },
  { id: "files", label: "Files", icon: FolderOpen },
  { id: "convert", label: "Convert", icon: ArrowLeftRight },
  { id: "homebrew", label: "Homebrew", icon: PackageIcon },
  { id: "saves", label: "Saves", icon: SaveIcon },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

const VERSION = "0.1.0";

export interface PageProps {
  snap: Snapshot;
  reload: () => void;
  toast: (message: string) => void;
  refreshKey: number;
  go: (page: string) => void;
}

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  const toast = useCallback((message: string) => {
    setToastMsg(message);
    window.setTimeout(() => setToastMsg((cur) => (cur === message ? null : cur)), 4600);
  }, []);

  const reload = useCallback(() => setRefreshKey((k) => k + 1), []);

  const loadSnapshot = useCallback(async () => {
    try {
      setSnap(await api.snapshot());
    } catch (err) {
      console.error("snapshot failed", err);
    }
  }, []);

  useEffect(() => {
    void loadSnapshot();
    const unlisteners: Promise<() => void>[] = [
      listen<{ id: number; done: number; total: number }>("job://progress", (update) => {
        setSnap((cur) =>
          cur
            ? {
                ...cur,
                jobs: cur.jobs.map((job) =>
                  job.id === update.id
                    ? { ...job, done_bytes: update.done, total_bytes: update.total }
                    : job
                ),
              }
            : cur
        );
      }),
      listen<Job>("job://job", (job) => {
        setSnap((cur) => {
          if (!cur) return cur;
          const exists = cur.jobs.some((j) => j.id === job.id);
          return {
            ...cur,
            jobs: exists
              ? cur.jobs.map((j) => (j.id === job.id ? job : j))
              : [job, ...cur.jobs],
          };
        });
        if (job.state === "done") toast(`${job.label}: ${job.message || "done"}`);
        else if (job.state === "failed") toast(`${job.label} failed: ${job.message}`);
        setRefreshKey((k) => k + 1);
      }),
    ];
    return () => {
      void Promise.all(unlisteners).then((uns) => uns.forEach((fn) => fn()));
    };
  }, [loadSnapshot, toast]);

  const activeStatus = useMemo(() => {
    if (!snap?.active) return null;
    return snap.statuses.find((s) => s.host_id === snap.active) ?? null;
  }, [snap]);

  const activeHost = useMemo(
    () => snap?.hosts.find((h) => h.id === snap.active) ?? null,
    [snap]
  );

  if (!snap) {
    return (
      <div className="shell" aria-busy="true">
        <div className="backdrop">
          <div className="orb cyan" />
          <div className="orb indigo" />
          <div className="orb brass" />
        </div>
      </div>
    );
  }

  const pageProps: PageProps = { snap, reload, toast, refreshKey, go: (p) => setPage(p as Page) };

  return (
    <div className="shell">
      <div className="backdrop" aria-hidden="true">
        <div className="orb cyan" />
        <div className="orb indigo" />
        <div className="orb brass" />
      </div>

      <a className="skip-link" href="#workspace">
        Skip to content
      </a>

      <header className="topbar">
        <div className="brand">
          <img className="mark" src={logo} alt="" aria-hidden="true" />
          <div>
            <div className="eyebrow">GODSEND PS4</div>
            <div className="page-title" aria-live="polite">
              {PAGE_TITLES[page]}
            </div>
          </div>
        </div>
        <div className="spacer" />
        {activeStatus ? (
          <Chip
            tone={activeStatus.ftp_ok ? "good" : "warn"}
            title={`${activeStatus.addr} - FTP ${activeStatus.ftp_ok ? "up" : "down"}`}
          >
            {activeHost?.name ?? activeStatus.name}
          </Chip>
        ) : (
          <Chip tone="bad">No console</Chip>
        )}
      </header>

      <nav className="rail" aria-label="Sections">
        <div className="rail-caption">Console</div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className={`rail-item ${page === item.id ? "on" : ""}`}
            aria-current={page === item.id ? "page" : undefined}
            onClick={() => setPage(item.id)}
            onKeyDown={(e) => {
              if (e.altKey && e.key >= "1" && e.key <= "9") {
                const idx = Number(e.key) - 1;
                if (NAV[idx]) setPage(NAV[idx].id);
              }
            }}
          >
            <item.icon className="glyph-icon" size={17} aria-hidden="true" />
            <span className="label">{item.label}</span>
          </button>
        ))}
      </nav>

      <main className="workspace with-inspector" id="workspace" tabIndex={-1}>
        {page === "home" && <Home {...pageProps} />}
        {page === "devices" && <Devices {...pageProps} />}
        {page === "library" && <Library {...pageProps} />}
        {page === "controller" && <Controller {...pageProps} />}
        {page === "store" && <GameStore {...pageProps} />}
        {page === "downloads" && <Downloads {...pageProps} />}
        {page === "files" && <Files {...pageProps} />}
        {page === "convert" && <Convert {...pageProps} />}
        {page === "homebrew" && <Homebrew {...pageProps} />}
        {page === "saves" && <Saves {...pageProps} />}
        {page === "settings" && <Settings {...pageProps} />}
      </main>

      <aside className="inspector" aria-label="Session">
        <div className="session-card">
          <h3>Device</h3>
          {activeStatus ? (
            <>
              <div className="kv">
                <span className="k">host</span>
                <span className="v" title={activeStatus.addr}>
                  {activeStatus.addr}
                </span>
              </div>
              <div className="kv">
                <span className="k">ftp</span>
                <span className="v">
                  {activeStatus.ftp_ok ? `${activeStatus.ftp_ms} ms` : "offline"}
                </span>
              </div>
              <div className="kv">
                <span className="k">server</span>
                <span className="v">{activeStatus.greeting ?? "-"}</span>
              </div>
              <div className="kv">
                <span className="k">installer</span>
                <span className="v">{activeStatus.rpi_ok ? "ready" : "missing"}</span>
              </div>
              <div className="kv">
                <span className="k">pad</span>
                <span className="v">{activeStatus.pad_ok ? "ready" : "missing"}</span>
              </div>
              <div className="kv">
                <span className="k">titles</span>
                <span className="v">{activeStatus.titles}</span>
              </div>
              <div className="kv">
                <span className="k">packages</span>
                <span className="v">{activeStatus.packages}</span>
              </div>
            </>
          ) : (
            <p style={{ margin: 0, fontSize: 12.5, color: "var(--text-faint)" }}>
              Add the console's IP on the Devices page. GoldHEN has to be running
              with its FTP service on (port 2121).
            </p>
          )}
        </div>

        <div className="queue-card">
          <div className="panel-head" style={{ marginBottom: 0 }}>
            <h3 style={{ margin: 0 }}>Queue</h3>
            <div className="spacer" />
            {snap.jobs.some((j) => j.state !== "running") ? (
              <button
                className="btn small"
                onClick={async () => {
                  await api.jobsClear();
                  void loadSnapshot();
                }}
              >
                Clear
              </button>
            ) : null}
          </div>
          <div className="rows">
            {snap.jobs.length === 0 ? (
              <p style={{ margin: 0, fontSize: 12.5, color: "var(--text-faint)" }}>
                No jobs yet. Transfers, installs and conversions show up here.
              </p>
            ) : (
              snap.jobs.slice(0, 8).map((job) => (
                <JobRow
                  key={job.id}
                  job={job}
                  onCancel={async () => {
                    await api.jobCancel(job.id);
                    void loadSnapshot();
                  }}
                />
              ))
            )}
          </div>
        </div>
      </aside>

      <footer className="statusbar">
        <span>
          <span className={`status-dot ${activeStatus?.ftp_ok ? "ok" : "off"}`} />
          {activeHost ? `${activeHost.name} (${activeHost.addr})` : "no console"}
        </span>
        <span>
          <span className={`status-dot ${activeStatus?.rpi_ok ? "ok" : "idle"}`} />
          installer {activeStatus?.rpi_ok ? "ready" : "missing"}
        </span>
        <span>
          <span className={`status-dot ${activeStatus?.pad_ok ? "ok" : "idle"}`} />
          pad {activeStatus?.pad_ok ? "ready" : "missing"}
        </span>
        <span className="grow" />
        <button className="btn small" onClick={() => setPage("downloads")}>
          Downloads
        </button>
        <span>godsend-ps4 v{VERSION}</span>
      </footer>

      {toastMsg ? (
        <div className="toast" role="status" aria-live="polite">
          <span className="chip laser">
            <span className="dot" />
            PS4
          </span>
          {toastMsg}
        </div>
      ) : null}
    </div>
  );
}

function JobRow({ job, onCancel }: { job: Job; onCancel: () => void }) {
  const running = job.state === "running";
  return (
    <div className="job-row">
      <div className="head">
        <span className="label" title={job.label}>
          {job.label}
        </span>
        {running ? (
          <button className="btn small" onClick={onCancel} title="Cancel job">
            Cancel
          </button>
        ) : (
          <Chip tone={job.state === "done" ? "good" : job.state === "failed" ? "bad" : "warn"}>
            {job.state}
          </Chip>
        )}
      </div>
      {running ? (
        <div style={{ marginTop: 6 }}>
          <div className="bar live">
            <div
              className="fill"
              style={{
                width: `${
                  job.total_bytes ? Math.round((job.done_bytes / job.total_bytes) * 100) : 12
                }%`,
              }}
            />
          </div>
        </div>
      ) : (
        <div className="msg">{job.message}</div>
      )}
    </div>
  );
}
