import { useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api, pickers } from "../api";
import type { Config } from "../types";
import { Btn, Toggle } from "../ui";

export default function Settings({ snap, toast, reload }: PageProps) {
  const [draft, setDraft] = useState<Config>(snap.config);
  const [saving, setSaving] = useState(false);

  useEffect(() => setDraft(snap.config), [snap.config]);

  async function save(patch: Partial<Config>) {
    setSaving(true);
    try {
      const next = await api.configSet(patch);
      setDraft(next);
      toast("settings saved");
      reload();
    } catch (err) {
      toast(`save failed: ${String(err)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Downloads and services</h2>
          <div className="spacer" />
          <Btn small disabled={saving} onClick={() => void save(draft)}>
            Save all
          </Btn>
        </div>

        <div className="grid two-col">
          <Field label="Download folder">
            <div className="quick">
              <input
                type="text"
                value={draft.download_dir}
                placeholder="default: ~/Downloads/godsend-ps4"
                onChange={(e) => setDraft({ ...draft, download_dir: e.target.value })}
              />
              <Btn
                small
                onClick={async () => {
                  const dir = await pickers.pickDirectory("Choose the download folder");
                  if (dir) setDraft({ ...draft, download_dir: dir });
                }}
              >
                Pick
              </Btn>
            </div>
          </Field>

          <Field label="aria2c path (optional)">
            <input
              type="text"
              className="mono"
              value={draft.aria2_path}
              placeholder="auto-detect"
              onChange={(e) => setDraft({ ...draft, aria2_path: e.target.value })}
            />
          </Field>

          <Field label="Real-Debrid API key">
            <input
              type="password"
              value={draft.rd_api_key}
              onChange={(e) => setDraft({ ...draft, rd_api_key: e.target.value })}
            />
          </Field>

          <Field label="TorBox API key">
            <input
              type="password"
              value={draft.tb_api_key}
              onChange={(e) => setDraft({ ...draft, tb_api_key: e.target.value })}
            />
          </Field>

          <Field label="Game search backend">
            <input
              type="text"
              className="mono"
              value={draft.gamesearch_base}
              onChange={(e) => setDraft({ ...draft, gamesearch_base: e.target.value })}
            />
          </Field>

          <Field label="Cover base (R2)">
            <input
              type="text"
              className="mono"
              value={draft.covers_base}
              onChange={(e) => setDraft({ ...draft, covers_base: e.target.value })}
            />
          </Field>

          <Field label="MCP server port (0 disables)">
            <input
              type="number"
              value={draft.mcp_port}
              onChange={(e) => setDraft({ ...draft, mcp_port: Number(e.target.value) })}
            />
          </Field>

          <Field label="FAT32 split size (MiB)">
            <input
              type="number"
              value={Math.round(draft.split_bytes / (1024 * 1024))}
              onChange={(e) =>
                setDraft({ ...draft, split_bytes: Math.max(1, Number(e.target.value)) * 1024 * 1024 })
              }
            />
          </Field>
        </div>

        <p style={{ fontSize: 12.5, color: "var(--text-faint)" }}>
          The MCP server listens on loopback only and starts with the app. Point
          an MCP client at <span className="mono">http://127.0.0.1:{draft.mcp_port}/mcp</span>.
        </p>
        <p style={{ fontSize: 12.5, color: "var(--text-faint)" }}>
          The game search backend is the Go service (port 3011 on the LAN box).
          The public site hides download links behind sign-in, so link resolution
          needs the backend.
        </p>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Behaviour</h2>
        </div>
        <div className="checks">
          <div className="setting-row">
            <div className="text">
              <div className="name">Install PKGs immediately</div>
              <div className="detail">Stream the package to the console installer as soon as the download finishes.</div>
            </div>
            <Toggle
              checked={draft.auto_install_pkg}
              label={draft.auto_install_pkg ? "on" : "off"}
              onChange={(next) => void save({ auto_install_pkg: next })}
            />
          </div>
          <div className="setting-row">
            <div className="text">
              <div className="name">Data folder</div>
              <div className="detail">Config, job journal and cached icons and keys.</div>
            </div>
            <Btn small onClick={() => void api.openDataDir()}>
              Open
            </Btn>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>About</h2>
        </div>
        <div className="rows">
          <div className="row">
            <div>
              <div className="title">GODsend PS4</div>
              <div className="sub mono">
                v0.1.0 - {snap.hosts.length} console{snap.hosts.length === 1 ? "" : "s"} configured
              </div>
            </div>
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
