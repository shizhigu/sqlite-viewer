import { useAppStore } from "../store/app";

export function ActivityPanel() {
  const open = useAppStore((s) => s.activityOpen);
  const activity = useAppStore((s) => s.activity);
  const setPushedQuery = useAppStore((s) => s.setPushedQuery);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const clearActivity = useAppStore((s) => s.clearActivity);
  const toggleActivity = useAppStore((s) => s.toggleActivity);

  if (!open) return null;

  const replay = (sql?: string) => {
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
        <span className="activity__count">{activity.length}</span>
        <span className="spacer" />
        <button
          className="btn"
          onClick={clearActivity}
          disabled={activity.length === 0}
          title="Clear log"
        >
          Clear
        </button>
        <button
          className="btn"
          onClick={toggleActivity}
          title="Hide panel (⌘⇧A)"
          style={{ marginLeft: 4 }}
        >
          ×
        </button>
      </div>
      <div className="activity__list">
        {activity.length === 0 ? (
          <div className="activity__empty">
            Nothing yet. Run <code>sqlv push "…"</code> from your terminal and
            it'll show up here.
          </div>
        ) : (
          [...activity].reverse().map((e) => (
            <div
              key={e.id}
              className={`activity__item ${e.error ? "activity__item--error" : ""}`}
            >
              <div className="activity__row">
                <span className="activity__kind">
                  {e.kind === "query" ? "query" : "open"}
                </span>
                <span className="activity__ts">{fmtTime(e.ts)}</span>
                {e.elapsed_ms !== undefined && (
                  <span className="activity__elapsed">{e.elapsed_ms} ms</span>
                )}
                {e.rows !== undefined && (
                  <span className="activity__elapsed">{e.rows} rows</span>
                )}
              </div>
              <div className="activity__sql mono">
                {e.sql ?? e.path ?? ""}
              </div>
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
