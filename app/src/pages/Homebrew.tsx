import { useCallback, useEffect, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { CatalogEntry } from "../types";
import { Btn, Chip, Empty, fmtBytes } from "../ui";

export default function Homebrew({ toast, reload, refreshKey }: PageProps) {
  const [entries, setEntries] = useState<CatalogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(
    async (refresh: boolean) => {
      setLoading(true);
      try {
        setEntries(await api.homebrewCatalog(refresh));
      } catch (err) {
        toast(`catalog failed: ${String(err)}`);
      } finally {
        setLoading(false);
      }
    },
    [toast]
  );

  useEffect(() => {
    void load(false);
  }, [load, refreshKey]);

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>PS4 homebrew</h2>
          <div className="spacer" />
          <Btn small disabled={loading} onClick={() => void load(true)}>
            {loading ? "Refreshing..." : "Refresh releases"}
          </Btn>
        </div>
        <p style={{ marginTop: 0, color: "var(--text-dim)", fontSize: 13 }}>
          Packages come from each project's latest GitHub release and install
          through the console's package installer. Control4Free is what the
          Controller page talks to.
        </p>
      </div>

      {entries.length === 0 ? (
        <Empty title="Catalog is empty" hint="Refresh to pull the latest releases." />
      ) : (
        <div className="store-grid">
          {entries.map((entry) => (
            <article className="store-card" key={entry.id}>
              <div className="head">
                <div className="name">{entry.name}</div>
                <span className="tag">{entry.category}</span>
              </div>
              <div className="desc">{entry.description}</div>
              {entry.note ? <div className="note">{entry.note}</div> : null}
              <div className="repo mono">{entry.repo}</div>
              <div className="foot">
                <span className="mono" style={{ fontSize: 11.5, color: "var(--text-faint)" }}>
                  {entry.latest_tag ?? "no release"}
                  {entry.size_bytes ? ` - ${fmtBytes(entry.size_bytes)}` : ""}
                </span>
                <div className="spacer" />
                {entry.asset_url ? (
                  <Btn
                    small
                    variant="primary"
                    onClick={async () => {
                      try {
                        await api.homebrewInstall(entry.id);
                        toast(`installing ${entry.name}`);
                        reload();
                      } catch (err) {
                        toast(`install failed: ${String(err)}`);
                      }
                    }}
                  >
                    Install
                  </Btn>
                ) : (
                  <Chip tone="warn">unavailable</Chip>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
