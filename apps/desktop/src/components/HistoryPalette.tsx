import { useEffect, useMemo, useState } from "react";

import { load as loadHistory, type HistoryEntry } from "../lib/history";
import { useAppStore } from "../store/app";

export interface HistoryPaletteProps {
  open: boolean;
  onClose: () => void;
  /** Called when the user picks an entry — parent should load it into editor. */
  onPick: (entry: HistoryEntry) => void;
}

export function HistoryPalette({ open, onClose, onPick }: HistoryPaletteProps) {
  const meta = useAppStore((s) => s.meta);
  const [filter, setFilter] = useState("");
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [cursor, setCursor] = useState(0);

  useEffect(() => {
    if (!open) return;
    setFilter("");
    setCursor(0);
    setEntries(loadHistory(meta?.path));
  }, [open, meta?.path]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const list = [...entries].reverse();
    if (!q) return list;
    return list.filter((e) => e.sql.toLowerCase().includes(q));
  }, [entries, filter]);

  useEffect(() => {
    if (cursor >= filtered.length) setCursor(Math.max(0, filtered.length - 1));
  }, [filtered.length, cursor]);

  if (!open) return null;

  const commit = (entry?: HistoryEntry) => {
    const target = entry ?? filtered[cursor];
    if (!target) return;
    onPick(target);
    onClose();
  };

  return (
    <div className="palette__backdrop" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          className="palette__input"
          placeholder="Filter query history…"
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
                ? "No history yet. Run some queries and they'll appear here."
                : "No matches."}
            </div>
          ) : (
            filtered.map((e, i) => (
              <button
                key={`${e.ts}-${i}`}
                className={`palette__item ${i === cursor ? "palette__item--active" : ""}`}
                onMouseEnter={() => setCursor(i)}
                onClick={() => commit(e)}
              >
                <div className="palette__sql mono">{truncate(e.sql, 140)}</div>
                <div className="palette__meta">
                  <span className="mono">{fmtDate(e.ts)}</span>
                  {e.elapsed_ms !== undefined && (
                    <span className="mono"> · {e.elapsed_ms} ms</span>
                  )}
                  {e.rows !== undefined && (
                    <span className="mono"> · {e.rows} rows</span>
                  )}
                  {e.error && <span className="palette__error">· error</span>}
                </div>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function truncate(s: string, n: number): string {
  const single = s.replace(/\s+/g, " ").trim();
  return single.length <= n ? single : single.slice(0, n - 1) + "…";
}

function fmtDate(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleString();
}
