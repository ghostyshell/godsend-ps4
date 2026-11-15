import { useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { Host } from "../types";
import { Btn, Chip, Empty, confirmDialog } from "../ui";

const EMPTY: Host = {
  id: "",
  name: "",
  addr: "",
  ftp_port: 2121,
  ftp_user: "anonymous",
  ftp_pass: "",
  rpi_port: 12800,
  pad_port: 4264,
  binloader_port: 9090,
};

export default function Devices({ snap, toast, reload }: PageProps) {
  const [draft, setDraft] = useState<Host>(EMPTY);
  const [editing, setEditing] = useState(false);
  const [subnet, setSubnet] = useState("192.168.1");

  const statusOf = (id: string) => snap.statuses.find((s) => s.host_id === id) ?? null;

  async function save() {
    if (!draft.addr.trim()) {
      toast("enter the console's IP address");
      return;
    }
    try {
      await api.hostAdd(draft);
      toast(`saved ${draft.name || draft.addr}`);
      setDraft(EMPTY);
      setEditing(false);
      reload();
    } catch (err) {
      toast(`save failed: ${String(err)}`);
    }
  }

  async function remove(host: Host) {
    if (!confirmDialog(`Remove ${host.name}?`, `${host.addr} stays configured on the console.`)) return;
    await api.hostRemove(host.id);
    reload();
  }

  async function discover() {
    try {
      toast(`scanning ${subnet}.1-254 over FTP`);
      const found = await api.discover(subnet);
      if (found.length === 0) {
        toast("no FTP servers answered on that subnet");
        return;
      }
      for (const host of found) {
        await api.hostAdd(host);
      }
      toast(`found ${found.length} console${found.length === 1 ? "" : "s"}`);
      reload();
    } catch (err) {
      toast(`discovery failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Consoles</h2>
          <div className="spacer" />
          <Btn
            variant="primary"
            onClick={() => {
              setDraft(EMPTY);
              setEditing(true);
            }}
          >
            Add console
          </Btn>
        </div>

        {snap.hosts.length === 0 ? (
          <Empty
            title="No console configured"
            hint="A PS4 running GoldHEN answers FTP on port 2121, the installer on 12800 and the pad on 4264."
          />
        ) : (
          <div className="rows">
            {snap.hosts.map((host) => {
              const status = statusOf(host.id);
              return (
                <div className="row" key={host.id}>
                  <span className={`status-dot ${status?.ftp_ok ? "ok" : "off"}`} />
                  <div>
                    <div className="title">{host.name}</div>
                    <div className="sub mono">
                      {host.addr} - ftp {host.ftp_port} as {host.ftp_user} - installer {host.rpi_port}
                      - pad {host.pad_port}
                    </div>
                  </div>
                  <div className="grow" />
                  {status?.rpi_ok ? <Chip tone="good">installer</Chip> : null}
                  {status?.pad_ok ? <Chip tone="good">pad</Chip> : null}
                  {snap.active === host.id ? <Chip tone="laser">active</Chip> : null}
                  <Btn small onClick={() => setDraft(host)}>Edit</Btn>
                  <Btn
                    small
                    onClick={async () => {
                      const probe = await api.probeHost(host.id);
                      toast(
                        probe.ftp_ok
                          ? `${probe.addr}: FTP ${probe.ftp_ms} ms, installer ${
                              probe.rpi_ok ? "ready" : "missing"
                            }, pad ${probe.pad_ok ? "ready" : "missing"}`
                          : `FTP failed: ${probe.message}`
                      );
                      reload();
                    }}
                  >
                    Test
                  </Btn>
                  {snap.active !== host.id ? (
                    <Btn
                      small
                      onClick={async () => {
                        await api.hostSetActive(host.id);
                        reload();
                      }}
                    >
                      Use
                    </Btn>
                  ) : null}
                  <Btn small variant="danger" onClick={() => void remove(host)}>
                    Remove
                  </Btn>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {(editing || draft.id) && (
        <div className="panel">
          <div className="panel-head">
            <h2>{draft.id ? "Edit console" : "New console"}</h2>
            <div className="spacer" />
            <Btn
              small
              onClick={() => {
                setDraft(EMPTY);
                setEditing(false);
              }}
            >
              Cancel
            </Btn>
          </div>
          <div className="grid two-col">
            <Field label="Name">
              <input
                type="text"
                value={draft.name}
                placeholder="Living room PS4"
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </Field>
            <Field label="Address">
              <input
                type="text"
                value={draft.addr}
                placeholder="192.168.1.50"
                onChange={(e) => setDraft({ ...draft, addr: e.target.value })}
              />
            </Field>
            <Field label="FTP port">
              <input
                type="number"
                value={draft.ftp_port}
                onChange={(e) => setDraft({ ...draft, ftp_port: Number(e.target.value) })}
              />
            </Field>
            <Field label="Installer port">
              <input
                type="number"
                value={draft.rpi_port}
                onChange={(e) => setDraft({ ...draft, rpi_port: Number(e.target.value) })}
              />
            </Field>
            <Field label="Virtual pad port">
              <input
                type="number"
                value={draft.pad_port}
                onChange={(e) => setDraft({ ...draft, pad_port: Number(e.target.value) })}
              />
            </Field>
            <Field label="Bin loader port">
              <input
                type="number"
                value={draft.binloader_port}
                onChange={(e) => setDraft({ ...draft, binloader_port: Number(e.target.value) })}
              />
            </Field>
            <Field label="FTP user">
              <input
                type="text"
                value={draft.ftp_user}
                onChange={(e) => setDraft({ ...draft, ftp_user: e.target.value })}
              />
            </Field>
            <Field label="FTP password">
              <input
                type="password"
                value={draft.ftp_pass}
                placeholder="empty on GoldHEN"
                onChange={(e) => setDraft({ ...draft, ftp_pass: e.target.value })}
              />
            </Field>
          </div>
          <div className="quick" style={{ marginTop: 14 }}>
            <Btn variant="primary" onClick={() => void save()}>
              Save console
            </Btn>
          </div>
        </div>
      )}

      <div className="panel">
        <div className="panel-head">
          <h2>Discover</h2>
          <div className="spacer" />
        </div>
        <p style={{ marginTop: 0, color: "var(--text-dim)", fontSize: 13 }}>
          Scans ports 2121 and 21 across the subnet, then keeps only hosts that
          answer as an FTP server.
        </p>
        <div className="grid two-col">
          <Field label="Subnet prefix">
            <input type="text" value={subnet} onChange={(e) => setSubnet(e.target.value)} />
          </Field>
          <div style={{ display: "flex", alignItems: "flex-end" }}>
            <Btn onClick={() => void discover()}>Scan the LAN</Btn>
          </div>
        </div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="field">
      <label>{label}</label>
      {children}
    </div>
  );
}
