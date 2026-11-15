import { useMemo, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import { Btn, Chip, Empty } from "../ui";

export default function Home({ snap, toast, reload, go }: PageProps) {
  const [busy, setBusy] = useState(false);
  const active = snap.statuses.find((s) => s.host_id === snap.active) ?? null;
  const host = snap.hosts.find((h) => h.id === snap.active) ?? null;
  const checks = useMemo(() => snap.doctor ?? [], [snap.doctor]);

  async function sendPayload() {
    setBusy(true);
    try {
      const path = await window.prompt("Path to a payload (.bin or .elf)");
      if (path) {
        await api.sendPayload(path);
        toast("payload sent to the bin loader");
        reload();
      }
    } catch (err) {
      toast(`payload failed: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <section className="hero">
        <div className="top">
          <div>
            <div className="eyebrow">CONSOLE</div>
            <h1>{host?.name ?? "No console selected"}</h1>
            <div className="sub">
              {active
                ? `${active.addr} - FTP ${active.ftp_ok ? `${active.ftp_ms} ms` : "offline"}${
                    active.greeting ? ` - ${active.greeting}` : ""
                  }`
                : "Add a PS4 on the Devices page to begin."}
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "flex-start" }}>
            {active ? (
              <>
                <Chip tone={active.ftp_ok ? "good" : "bad"} live={active.ftp_ok}>
                  {active.ftp_ok ? "FTP online" : "FTP offline"}
                </Chip>
                <Chip tone={active.rpi_ok ? "good" : "warn"}>
                  {active.rpi_ok ? "installer" : "no installer"}
                </Chip>
                <Chip tone={active.pad_ok ? "good" : "warn"}>
                  {active.pad_ok ? "pad" : "no pad"}
                </Chip>
              </>
            ) : null}
          </div>
        </div>

        <div className="stats">
          <Stat k="server" v={active?.greeting?.split(" ")[0] ?? "-"} />
          <Stat k="titles" v={active ? String(active.titles) : "-"} />
          <Stat k="packages" v={active ? String(active.packages) : "-"} />
          <Stat k="ftp" v={active?.ftp_ok ? `${active.ftp_ms} ms` : "-"} />
        </div>

        <div className="quick">
          <Btn variant="primary" disabled={!active?.ftp_ok || busy} onClick={() => go("library")}>
            Open library
          </Btn>
          <Btn disabled={!active?.ftp_ok || busy} onClick={() => go("controller")}>
            Virtual pad
          </Btn>
          <Btn disabled={!active?.ftp_ok || busy} onClick={() => go("downloads")}>
            Downloads
          </Btn>
          <Btn disabled={!active?.ftp_ok || busy} onClick={() => go("homebrew")}>
            Homebrew
          </Btn>
          <Btn disabled={!active || busy} onClick={sendPayload} title="Send a payload over port 9090">
            Send payload
          </Btn>
          <Btn disabled={!active || busy} onClick={() => reload()}>
            Rescan
          </Btn>
        </div>
      </section>

      <div className="panel">
        <div className="panel-head">
          <h2>Preflight</h2>
          <div className="spacer" />
          <Btn small disabled={busy} onClick={() => reload()}>
            Re-check
          </Btn>
        </div>
        <div className="checks">
          {checks.map((check) => (
            <div className="check" key={check.name}>
              <span className={`status-dot ${check.ok ? "ok" : "idle"}`} />
              <div>
                <div className="name">{check.name}</div>
                <div className="detail">{check.detail}</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Consoles</h2>
          <div className="spacer" />
          <Btn small onClick={() => go("devices")}>
            Manage
          </Btn>
        </div>
        {snap.statuses.length === 0 ? (
          <Empty
            title="No consoles yet"
            hint="Add the PS4's LAN address. GoldHEN serves FTP on port 2121."
            action={
              <Btn variant="primary" onClick={() => go("devices")}>
                Add a console
              </Btn>
            }
          />
        ) : (
          <div className="rows">
            {snap.statuses.map((status) => (
              <div className="row" key={status.host_id}>
                <span className={`status-dot ${status.ftp_ok ? "ok" : "off"}`} />
                <div>
                  <div className="title">{status.name}</div>
                  <div className="sub">
                    {status.addr} - {status.greeting ?? "no FTP banner"}
                  </div>
                </div>
                <div className="grow" />
                <span className="size mono">
                  {status.ftp_ok ? `${status.titles} titles` : "offline"}
                </span>
                {status.host_id !== snap.active ? (
                  <Btn
                    small
                    onClick={async () => {
                      await api.hostSetActive(status.host_id);
                      toast(`active console: ${status.name}`);
                      reload();
                    }}
                  >
                    Use
                  </Btn>
                ) : (
                  <Chip tone="laser">active</Chip>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Install and pad</h2>
        </div>
        <div className="checks">
          <div className="check">
            <span className={`status-dot ${active?.rpi_ok ? "ok" : "idle"}`} />
            <div>
              <div className="name">Package installer</div>
              <div className="detail">
                GoldHEN answers on port {snap.hosts.find((h) => h.id === snap.active)?.rpi_port ?? 12800}
                . Packages are streamed straight to it, so a 40 GB title never touches the console's
                storage twice.
              </div>
            </div>
          </div>
          <div className="check">
            <span className={`status-dot ${active?.pad_ok ? "ok" : "idle"}`} />
            <div>
              <div className="name">Virtual pad</div>
              <div className="detail">
                {active?.pad_ok
                  ? `Control4Free is loaded${
                      snap.pad?.protocol ? ` (protocol ${snap.pad.protocol}, ${snap.pad.pads ?? 1} pads)` : ""
                    }.`
                  : "Install Control4Free from the Homebrew page, then open the Controller."}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Stat({ k, v }: { k: string; v: string }) {
  return (
    <div className="stat">
      <div className="v">{v}</div>
      <div className="k">{k}</div>
    </div>
  );
}
