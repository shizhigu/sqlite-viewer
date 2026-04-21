import { useEffect, useState } from "react";

import type { ActivityRecord } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

export function ActivityPanel() {
  const open = useAppStore((s) => s.activityOpen);
  const liveFeed = useAppStore((s) => s.activity);
  const setPushedQuery = useAppStore((s) => s.setPushedQuery);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const clearLive = useAppStore((s) => s.clearActivity);
  const toggleActivity = useAppStore((s) => s.toggleActivity);
  const meta = useAppStore((s) => s.meta);

  // The "persistent" tab surfaces ~/.sqlv/activity.db so users see what
  // happened across past sessions and across the CLI + MCP surfaces.
  const [mode, setMode] = useState<"live" | "persistent">("live");
  const [persistent, setPersistent] = useState<ActivityRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [grep, setGrep] = useState("");
  const [scopeToDb, setScopeToDb] = useState(true);

  useEffect(() => {
    if (!open || mode !== "persistent") return;
    let cancelled = false;
    setLoading(true);
    tauri
      .activityQuery({
        grep: grep.trim() ? grep.trim() : null,
        db_path: scopeToDb ? (meta?.path ?? null) : null,
        limit: 500,
      })
      .then((r) => {
        if (!cancelled) setPersistent(r.records);
      })
      .catch(() => {
        if (!cancelled) setPersistent([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, mode, grep, scopeToDb, meta?.path]);

  if (!open) return null;

  const replay = (sql?: string | null) => {
    if (!sql) return;
    setPushedQuery({
      sql,
      result: null,
      error: null,
      token: Date.now(),
    });
    setActiveTab("query");
  };

  return (
    <aside className="activity">
      <div className="activity__header">
        <strong>Agent activity</strong>
        <div className="activity__tabs">
          <button
            className={`activity__tab ${mode === "live" ? "activity__tab--active" : ""}`}
            onClick={() => setMode("live")}
          >
            Live ({liveFeed.length})
          </button>
          <button
            className={`activity__tab ${mode === "persistent" ? "activity__tab--active" : ""}`}
            onClick={() => setMode("persistent")}
            title="Shared ~/.sqlv/activity.db — CLI + MCP + UI history"
          >
            Persistent
          </button>
        </div>
        <span className="spacer" />
        {mode === "live" ? (
          <button
            className="btn"
            onClick={clearLive}
            disabled={liveFeed.length === 0}
            title="Clear live feed (doesn't touch persistent log)"
          >
            Clear
          </button>
        ) : (
          <button
            className="btn"
            onClick={() =>
              tauri.activityPrune(Date.now() - 7 * 24 * 60 * 60 * 1000)
            }
            title="Prune entries older than 7 days"
          >
            Prune 7d
          </button>
        )}
        <button
          className="btn"
          onClick={toggleActivity}
          title="Hide panel (⌘⇧A)"
          style={{ marginLeft: 4 }}
        >
          ×
        </button>
      </div>

      {mode === "persistent" && (
        <div className="activity__controls">
          <input
            className="activity__search"
            placeholder="grep sql or db_path…"
            value={grep}
            onChange={(e) => setGrep(e.target.value)}
          />
          <label className="activity__scope">
            <input
              type="checkbox"
              checked={scopeToDb}
              onChange={(e) => setScopeToDb(e.target.checked)}
              disabled={!meta}
            />
            only this DB
          </label>
        </div>
      )}

      <div className="activity__list">
        {mode === "live" ? (
          liveFeed.length === 0 ? (
            <div className="activity__empty">
              Nothing yet. Run <code>sqlv push "…"</code> from your terminal and
              it'll show up here.
            </div>
          ) : (
            [...liveFeed].reverse().map((e) => (
              <div
                key={e.id}
                className={`activity__item ${e.error ? "activity__item--error" : ""}`}
              >
                <div className="activity__row">
                  <span className="activity__kind">{e.kind}</span>
                  <span className="activity__ts">{fmtTime(e.ts)}</span>
                  {e.elapsed_ms !== undefined && (
                    <span className="activity__elapsed">{e.elapsed_ms} ms</span>
                  )}
                  {e.rows !== undefined && (
                    <span className="activity__elapsed">{e.rows} rows</span>
                  )}
                </div>
                <div className="activity__sql mono">{e.sql ?? e.path ?? ""}</div>
                {e.error && (
                  <div className="activity__error mono">
                    [{e.error.code}] {e.error.message}
                  </div>
                )}
                {e.kind === "query" && e.sql && !e.error && (
                  <div className="activity__actions">
                    <button
                      className="btn"
                      onClick={() => replay(e.sql)}
                      title="Replay this query in the editor"
                    >
                      ▶ Replay
                    </button>
                  </div>
                )}
              </div>
            ))
          )
        ) : loading ? (
          <div className="activity__empty">Loading…</div>
        ) : persistent.length === 0 ? (
          <div className="activity__empty">
            Nothing in <code>~/.sqlv/activity.db</code> yet.
          </div>
        ) : (
          persistent.map((r) => (
            <div
              key={r.id}
              className={`activity__item ${r.error_code ? "activity__item--error" : ""}`}
            >
              <div className="activity__row">
                <span className="activity__kind">{r.kind}</span>
                <span className="activity__source">{r.source}</span>
                <span className="activity__ts">{fmtTime(r.ts_ms)}</span>
                {r.elapsed_ms !== null && (
                  <span className="activity__elapsed">{r.elapsed_ms} ms</span>
                )}
                {r.rows !== null && (
                  <span className="activity__elapsed">{r.rows} rows</span>
                )}
              </div>
              <div className="activity__sql mono">
                {r.sql ?? r.db_path ?? ""}
              </div>
              {r.error_code && (
                <div className="activity__error mono">
                  [{r.error_code}] {r.error_message}
                </div>
              )}
              {r.kind === "query" && r.sql && !r.error_code && (
                <div className="activity__actions">
                  <button
                    className="btn"
                    onClick={() => replay(r.sql)}
                    title="Replay this query in the editor"
                  >
                    ▶ Replay
                  </button>
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </aside>
  );
}

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}
