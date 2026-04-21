import { useState } from "react";

import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

export interface StagedChangesPanelProps {
  /** Called after a successful commit so the grid can refetch. */
  onCommitted?: () => void;
}

export function StagedChangesPanel({ onCommitted }: StagedChangesPanelProps) {
  const staged = useAppStore((s) => s.stagedChanges);
  const remove = useAppStore((s) => s.removeStagedChange);
  const clear = useAppStore((s) => s.clearStagedChanges);
  const pushToast = useAppStore((s) => s.pushToast);
  const pushError = useAppStore((s) => s.pushError);
  const [committing, setCommitting] = useState(false);
  const [collapsed, setCollapsed] = useState(false);

  if (staged.length === 0) return null;

  const commitAll = async () => {
    if (staged.length === 0) return;
    setCommitting(true);
    try {
      const statements: [string, typeof staged[number]["params"]][] = staged.map(
        (c) => [c.sql, c.params],
      );
      const res = await tauri.runExecMany(statements);
      pushToast(
        "success",
        `Committed ${staged.length} change(s) · ${res.rows_affected} rows affected`,
      );
      clear();
      onCommitted?.();
    } catch (e) {
      pushError(e as string);
    } finally {
      setCommitting(false);
    }
  };

  const byTable = staged.reduce<Record<string, number>>((m, c) => {
    m[c.table] = (m[c.table] ?? 0) + 1;
    return m;
  }, {});

  return (
    <div className={`staged ${collapsed ? "staged--collapsed" : ""}`}>
      <div className="staged__header">
        <strong>Staged changes</strong>
        <span className="chip staged__count">{staged.length}</span>
        <span className="staged__tables">
          {Object.entries(byTable)
            .map(([t, n]) => `${t} (${n})`)
            .join(" · ")}
        </span>
        <span className="spacer" />
        <button
          className="btn"
          onClick={() => setCollapsed((c) => !c)}
          title={collapsed ? "Expand" : "Collapse"}
        >
          {collapsed ? "▴" : "▾"}
        </button>
        <button
          className="btn"
          onClick={clear}
          disabled={committing}
          title="Discard all pending changes"
        >
          Discard all
        </button>
        <button
          className="btn btn--primary"
          onClick={commitAll}
          disabled={committing}
          title="Commit every pending change in one transaction"
        >
          {committing ? "Committing…" : `Commit all (${staged.length})`}
        </button>
      </div>
      {!collapsed && (
        <div className="staged__list">
          {staged.map((c) => (
            <div key={c.id} className={`staged__item staged__item--${c.op}`}>
              <span className={`staged__op staged__op--${c.op}`}>{c.op}</span>
              <span className="staged__table mono">{c.table}</span>
              <span className="staged__summary">{c.summary}</span>
              <span className="spacer" />
              <button
                className="btn"
                onClick={() => remove(c.id)}
                disabled={committing}
                title="Revert this change"
              >
                Revert
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
