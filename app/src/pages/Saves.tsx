import { useCallback, useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api, pickers } from "../api";
import type { SaveEntry } from "../types";
import { Btn, Empty, confirmDialog, fmtBytes } from "../ui";

export default function Saves({ toast, reload, refreshKey }: PageProps) {
  const [saves, setSaves] = useState<SaveEntry[] | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setSaves(await api.savesList());
    } catch (err) {
      toast(`save scan failed: ${String(err)}`);
      setSaves([]);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  function toggle(titleId: string) {
    setSelected((cur) =>
      cur.includes(titleId) ? cur.filter((id) => id !== titleId) : [...cur, titleId]
    );
  }

  async function backup() {
    if (selected.length === 0) {
      toast("select at least one save");
      return;
    }
    try {
      await api.savesBackup(selected);
      toast(`backing up ${selected.length} save(s) to Downloads`);
      reload();
    } catch (err) {
      toast(`backup failed: ${String(err)}`);
    }
  }

  async function restore() {
    const zip = await pickers.pickFile("Choose a save backup", ["zip"]);
    if (!zip) return;
    try {
      await api.savesRestore(zip);
      toast("restoring save data");
      reload();
    } catch (err) {
      toast(`restore failed: ${String(err)}`);
    }
  }

  async function remove(entry: SaveEntry) {
    if (
      !confirmDialog(
        `Delete the save for ${entry.title ?? entry.title_id}?`,
        `Removes ${entry.path} and every file in it. This cannot be undone.`
      )
    ) {
      return;
    }
    try {
      await api.savesDelete(entry.path);
      toast("save deleted");
      void load();
    } catch (err) {
      toast(`delete failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Save data</h2>
          <div className="spacer" />
          <Btn small disabled={loading} onClick={() => void load()}>
            {loading ? "Scanning..." : "Rescan"}
          </Btn>
          <Btn small variant="primary" disabled={selected.length === 0} onClick={() => void backup()}>
            Back up selected
          </Btn>
          <Btn small onClick={() => void restore()}>
            Restore
          </Btn>
        </div>

        {saves === null ? (
          <Empty title="Scanning the console..." />
        ) : saves.length === 0 ? (
          <Empty
            title="No save data found"
            hint="Saves live under /user/home/<user>/savedata. Play a game first, or check that GoldHEN exposes /user/home."
          />
        ) : (
          <div className="rows">
            {saves.map((save) => (
              <div className="row" key={save.path}>
                <input
                  type="checkbox"
                  checked={selected.includes(save.title_id)}
                  onChange={() => toggle(save.title_id)}
                  aria-label={`Select ${save.title ?? save.title_id}`}
                />
                <div>
                  <div className="title">{save.title ?? save.title_id}</div>
                  <div className="sub mono">
                    {save.title_id} - user {save.user} - {save.files} file
                    {save.files === 1 ? "" : "s"}
                  </div>
                </div>
                <div className="grow" />
                <span className="size mono">{fmtBytes(save.size_bytes)}</span>
                <Btn small variant="danger" onClick={() => void remove(save)}>
                  Delete
                </Btn>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>How backups work</h2>
        </div>
        <div className="checks">
          <div className="check">
            <span className="status-dot idle" />
            <div>
              <div className="name">Back up</div>
              <div className="detail">
                Zips the selected save folders into Downloads as ps4-saves-&lt;timestamp&gt;.zip. Each
                entry keeps the title id as its top folder.
              </div>
            </div>
          </div>
          <div className="check">
            <span className="status-dot idle" />
            <div>
              <div className="name">Restore</div>
              <div className="detail">
                Uploads the zip's entries back to /user/home/&lt;first user&gt;/savedata/&lt;title id&gt;.
                Close the game first: the console caches save files while it runs.
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
