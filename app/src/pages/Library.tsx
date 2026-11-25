import { useCallback, useEffect, useMemo, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { GameEntry, LibraryView } from "../types";
import { Btn, Chip, Empty, GameIcon, Seg, confirmDialog, fmtBytes } from "../ui";

type Filter = "all" | "game" | "homebrew" | "pkg";

export default function Library({ toast, refreshKey, go }: PageProps) {
  const [view, setView] = useState<LibraryView | null>(null);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setView(await api.libraryScan());
    } catch (err) {
      toast(`library scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  const entries = useMemo(() => {
    if (!view) return [];
    const all = [...view.games, ...view.packages];
    return all.filter((entry) => {
      if (filter === "pkg" && entry.kind !== "pkg") return false;
      if (filter === "game" && entry.kind !== "app") return false;
      if (filter === "homebrew" && entry.kind !== "homebrew") return false;
      if (query.trim() && !entry.title.toLowerCase().includes(query.trim().toLowerCase())) {
        return false;
      }
      return true;
    });
  }, [view, filter, query]);

  async function remove(entry: GameEntry) {
    if (
      !confirmDialog(
        `Delete ${entry.title}?`,
        `Removes ${entry.path} from the console. This cannot be undone.`
      )
    ) {
      return;
    }
    try {
      await api.libraryDelete(entry.path, entry.kind !== "pkg");
      toast(`deleted ${entry.title}`);
      void load();
    } catch (err) {
      toast(`delete failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Installed library</h2>
          <div className="spacer" />
          <input
            className="search"
            type="text"
            aria-label="Filter by title"
            placeholder="Filter by title"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <Btn small disabled={loading} onClick={() => void load()}>
            {loading ? "Scanning..." : "Rescan"}
          </Btn>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
          <Seg<Filter>
            ariaLabel="Library filter"
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: "All" },
              { value: "game", label: "Games" },
              { value: "homebrew", label: "Homebrew" },
              { value: "pkg", label: "Packages" },
            ]}
          />
          <span className="mono" style={{ color: "var(--text-faint)", fontSize: 12 }}>
            {view ? `${view.games.length} titles - ${view.packages.length} packages` : "not scanned"}
          </span>
        </div>
      </div>

      {entries.length === 0 ? (
        <Empty
          title={loading ? "Scanning the console..." : "Nothing here yet"}
          hint="Titles in /user/appmeta and packages in /user/pkg appear here. Install a PKG from Downloads or Homebrew."
          action={
            <Btn variant="primary" onClick={() => go("downloads")}>
              Send something to the console
            </Btn>
          }
        />
      ) : (
        <div className="grid">
          {entries.map((entry) => (
            <article className="tile" key={entry.id}>
              <div className="cover">
                <GameIcon path={entry.icon_path} title={entry.title} />
              </div>
              <div className="body">
                <div className="t" title={entry.title}>
                  {entry.title}
                </div>
                <div className="s">
                  {entry.kind === "pkg"
                    ? "package"
                    : entry.category === "homebrew"
                    ? "homebrew"
                    : "game"}{" "}
                  - {entry.version ? `v${entry.version}` : entry.device}
                </div>
                <div className="actions">
                  {entry.kind === "pkg" ? (
                    <>
                      <Chip tone="warn">waiting</Chip>
                      <Btn small variant="danger" onClick={() => void remove(entry)}>
                        Delete
                      </Btn>
                    </>
                  ) : (
                    <>
                      <Chip tone="good">installed</Chip>
                      <span className="size mono">
                        {entry.size_bytes ? fmtBytes(entry.size_bytes) : entry.device}
                      </span>
                    </>
                  )}
                </div>
                <div className="s mono" title={entry.path}>
                  {entry.title_id ?? entry.path.split("/").pop()}
                </div>
              </div>
            </article>
          ))}
        </div>
      )}

      {view && view.roots.length > 0 ? (
        <p className="mono" style={{ color: "var(--text-faint)", fontSize: 11.5 }}>
          scanned: {view.roots.join("  ")}
        </p>
      ) : null}
    </div>
  );
}
