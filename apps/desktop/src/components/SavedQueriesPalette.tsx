import { useEffect, useMemo, useState } from "react";

import { load as loadSaved, remove as removeSaved, type SavedQuery } from "../lib/saved";
import { useAppStore } from "../store/app";

export interface SavedQueriesPaletteProps {
  open: boolean;
  onClose: () => void;
  onPick: (entry: SavedQuery) => void;
}

export function SavedQueriesPalette({ open, onClose, onPick }: SavedQueriesPaletteProps) {
  const meta = useAppStore((s) => s.meta);
  const [filter, setFilter] = useState("");
  const [entries, setEntries] = useState<SavedQuery[]>([]);
  const [cursor, setCursor] = useState(0);

  useEffect(() => {
    if (!open) return;
    setFilter("");
    setCursor(0);
    setEntries(loadSaved(meta?.path));
  }, [open, meta?.path]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter(
      (e) =>
        e.name.toLowerCase().includes(q) || e.sql.toLowerCase().includes(q),
    );
  }, [entries, filter]);

  if (!open) return null;

  const commit = (entry?: SavedQuery) => {
    const target = entry ?? filtered[cursor];
    if (!target) return;
    onPick(target);
    onClose();
  };

  const deleteAt = (name: string) => {
    setEntries(removeSaved(meta?.path, name));
  };

  return (
    <div className="palette__backdrop" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          className="palette__input"
          placeholder="Filter saved queries…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onClose();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setCursor((c) => Math.min(filtered.length - 1, c + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setCursor((c) => Math.max(0, c - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              commit();
            }
          }}
        />
        <div className="palette__hint">
          <kbd>↑↓</kbd> navigate · <kbd>⏎</kbd> insert · <kbd>Esc</kbd> close ·{" "}
          {filtered.length}/{entries.length}
        </div>
        <div className="palette__list">
          {filtered.length === 0 ? (
            <div className="palette__empty">
              {entries.length === 0
                ? "No saved queries yet. Click Save in the Query tab to bookmark one."
                : "No matches."}
            </div>
          ) : (
            filtered.map((e, i) => (
              <div
                key={`${e.name}-${e.ts}`}
                className={`palette__item ${i === cursor ? "palette__item--active" : ""}`}
                onMouseEnter={() => setCursor(i)}
              >
                <button
                  className="palette__item-main"
                  onClick={() => commit(e)}
                  title="Load into editor"
                >
                  <div className="palette__sql mono">
                    <strong style={{ fontFamily: "var(--font-ui)", marginRight: 8 }}>
                      {e.name}
                    </strong>
                    {truncate(e.sql, 110)}
                  </div>
                  <div className="palette__meta">
                    <span className="mono">{new Date(e.ts).toLocaleString()}</span>
                  </div>
                </button>
                <button
                  className="btn btn--danger"
                  style={{ height: 26, marginLeft: 8 }}
                  onClick={(ev) => {
                    ev.stopPropagation();
                    deleteAt(e.name);
                  }}
                  title="Delete this saved query"
                >
                  ×
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function truncate(s: string, n: number): string {
  const one = s.replace(/\s+/g, " ").trim();
  return one.length <= n ? one : one.slice(0, n - 1) + "…";
}
