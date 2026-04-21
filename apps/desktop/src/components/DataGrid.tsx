import { useEffect, useMemo, useRef, useState } from "react";

import {
  buildDelete,
  buildInsert,
  buildUpdate,
  coerceFromString,
  formatValue,
} from "../lib/sql";
import type { Column, ForeignKey, QueryResult, TableSchema, Value } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

const PAGE_SIZE = 100;

export interface DataGridProps {
  schema: TableSchema;
  /** Total row count, if known (from `tables` metadata). Used to render
   * `N–M of TOTAL` and to disable `Next` on the last page. */
  totalRows: number | null;
  /** Called when a mutation changes the server state — parent should refetch. */
  onMutated: () => void;
}

export function DataGrid({ schema, totalRows, onMutated }: DataGridProps) {
  const readWrite = useAppStore((s) => s.readWrite);
  const pushError = useAppStore((s) => s.pushError);
  const pushToast = useAppStore((s) => s.pushToast);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const [page, setPage] = useState(0);
  const [result, setResult] = useState<QueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  const pkCols = useMemo(
    () => schema.columns.filter((c) => c.pk > 0).sort((a, b) => a.pk - b.pk),
    [schema],
  );

  // Map of column name → FK target (for clickable ↗ links).
  const fkByColumn = useMemo(() => {
    const m: Record<string, ForeignKey> = {};
    for (const fk of schema.foreign_keys) m[fk.from] = fk;
    return m;
  }, [schema]);

  const followFk = async (fk: ForeignKey, _value: Value) => {
    try {
      const s = await tauri.describeTable(fk.table);
      setSelectedTable(fk.table);
      setSelectedSchema(s);
      pushToast("info", `→ ${fk.table}.${fk.to}`);
    } catch (e) {
      pushError(e as string);
    }
  };

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      setLoading(true);
      try {
        const res = await tauri.runQuery(
          `SELECT * FROM ${quote(schema.name)}`,
          [],
          PAGE_SIZE,
          page * PAGE_SIZE,
        );
        if (!cancelled) setResult(res);
      } catch (e) {
        if (!cancelled) pushError(e as string);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [schema.name, page, pushError]);

  const commitCell = async (
    rowIdx: number,
    col: Column,
    rawInput: string,
  ) => {
    if (!result) return;
    const row = result.rows[rowIdx];
    const allowNullOnEmpty = !col.not_null;
    let newValue: Value;
    try {
      newValue = coerceFromString(rawInput, col.decl_type, allowNullOnEmpty);
    } catch (e) {
      pushError((e as Error).message);
      return;
    }
    if (pkCols.length === 0) {
      pushError(
        "Cannot edit — this table has no primary key and inline edits aren't supported without one.",
      );
      return;
    }
    const pk: Record<string, Value> = {};
    for (const pkCol of pkCols) {
      const i = result.columns.indexOf(pkCol.name);
      pk[pkCol.name] = row[i];
    }
    const { sql, params } = buildUpdate(
      schema.name,
      { [col.name]: newValue },
      pk,
    );
    try {
      const res = await tauri.runExec(sql, params);
      if (res.rows_affected !== 1) {
        pushToast("info", `No row affected (${res.rows_affected})`);
      }
      onMutated();
    } catch (e) {
      pushError(e as string);
    }
  };

  const addEmptyRow = async () => {
    if (!readWrite) return;
    try {
      const { sql, params } = buildInsert(schema.name, {});
      await tauri.runExec(sql, params);
      pushToast("success", `Added row`);
      onMutated();
    } catch (e) {
      pushError(e as string);
    }
  };

  const deleteSelected = async () => {
    if (!readWrite || selected.size === 0 || !result) return;
    if (pkCols.length === 0) {
      pushError("Cannot delete — this table has no primary key.");
      return;
    }
    // Build all the DELETEs up front and submit them as one transaction.
    // A constraint failure on row N rolls back rows 1..N-1 — no half-deleted
    // state.
    const statements: [string, Value[]][] = [];
    for (const idx of selected) {
      const row = result.rows[idx];
      const pk: Record<string, Value> = {};
      for (const pkCol of pkCols) {
        const i = result.columns.indexOf(pkCol.name);
        pk[pkCol.name] = row[i];
      }
      const { sql, params } = buildDelete(schema.name, pk);
      statements.push([sql, params]);
    }
    try {
      const res = await tauri.runExecMany(statements);
      pushToast("success", `Deleted ${res.rows_affected} row(s)`);
      setSelected(new Set());
      onMutated();
    } catch (e) {
      pushError(e as string);
    }
  };

  if (loading && !result) {
    return <div className="grid__empty">Loading…</div>;
  }
  if (!result) {
    return <div className="grid__empty">No data.</div>;
  }
  if (result.rows.length === 0 && page === 0) {
    return (
      <div className="grid">
        <div className="grid__empty">
          No rows in <code>{schema.name}</code>.
          {readWrite && (
            <div style={{ marginTop: 12 }}>
              <button className="btn btn--primary" onClick={addEmptyRow}>
                + Add row
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  const startRow = page * PAGE_SIZE + 1;
  const endRow = page * PAGE_SIZE + result.rows.length;
  // Prefer the known total; fall back to "≥endRow" when the row count isn't
  // available (e.g. when browsing a view).
  const totalLabel =
    totalRows !== null ? totalRows.toLocaleString() : `≥ ${endRow}`;
  const maxPage =
    totalRows !== null ? Math.max(0, Math.ceil(totalRows / PAGE_SIZE) - 1) : Infinity;
  const atLastPage = totalRows !== null ? page >= maxPage : !result.truncated;

  return (
    <div className="grid">
      <div className="grid__scroll">
        <table>
          <thead>
            <tr>
              <th style={{ width: 30 }}>#</th>
              {schema.columns.map((col) => (
                <th key={col.name}>
                  <span>
                    {col.pk > 0 && <span className="col-badge">⚷</span>}
                    {col.name}
                  </span>
                  <span className="col-type">
                    {col.decl_type ?? ""} {col.not_null ? "· NOT NULL" : ""}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {result.rows.map((row, rowIdx) => (
              <tr
                key={rowIdx}
                className={selected.has(rowIdx) ? "grid-row--selected" : ""}
                onClick={(e) => toggleSelected(rowIdx, e.shiftKey, e.metaKey, selected, setSelected)}
              >
                <td style={{ color: "var(--text-muted)" }}>
                  {page * PAGE_SIZE + rowIdx + 1}
                </td>
                {schema.columns.map((col, i) => (
                  <Cell
                    key={col.name}
                    value={row[i]}
                    column={col}
                    editable={readWrite && col.pk === 0}
                    fk={fkByColumn[col.name]}
                    onFollowFk={(fk) => followFk(fk, row[i])}
                    onCommit={(raw) => commitCell(rowIdx, col, raw)}
                  />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="grid__footer">
        {selected.size > 0 && <span>{selected.size} selected</span>}
        <span className="spacer" />
        <button
          className="btn"
          onClick={() => setPage((p) => Math.max(0, p - 1))}
          disabled={page === 0}
          aria-label="Previous page"
        >
          ◂
        </button>
        <span className="mono" style={{ fontSize: "var(--text-xs)" }}>
          {startRow.toLocaleString()}–{endRow.toLocaleString()} of {totalLabel}
        </span>
        <button
          className="btn"
          onClick={() => setPage((p) => p + 1)}
          disabled={atLastPage}
          aria-label="Next page"
        >
          ▸
        </button>
        <span className="spacer" />
        <button
          className="btn btn--primary"
          onClick={addEmptyRow}
          disabled={!readWrite}
        >
          + Add row
        </button>
        <button
          className="btn btn--danger"
          onClick={deleteSelected}
          disabled={!readWrite || selected.size === 0}
        >
          − Delete selected
        </button>
      </div>
    </div>
  );
}

function toggleSelected(
  rowIdx: number,
  shift: boolean,
  meta: boolean,
  prev: Set<number>,
  set: (n: Set<number>) => void,
) {
  if (meta || shift) {
    const next = new Set(prev);
    if (next.has(rowIdx)) next.delete(rowIdx);
    else next.add(rowIdx);
    set(next);
  } else {
    set(new Set([rowIdx]));
  }
}

function Cell({
  value,
  column,
  editable,
  fk,
  onFollowFk,
  onCommit,
}: {
  value: Value;
  column: Column;
  editable: boolean;
  fk?: ForeignKey;
  onFollowFk?: (fk: ForeignKey) => void;
  onCommit: (raw: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(() => initialDraft(value));
  const [invalid, setInvalid] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const begin = () => {
    if (!editable) return;
    setDraft(initialDraft(value));
    setInvalid(false);
    setEditing(true);
  };

  const commit = () => {
    setEditing(false);
    try {
      coerceFromString(draft, column.decl_type, !column.not_null);
    } catch {
      setInvalid(true);
      return;
    }
    onCommit(draft);
  };

  const cancel = () => {
    setEditing(false);
    setInvalid(false);
  };

  const isNull = value === null;
  const isBlob =
    typeof value === "object" && value !== null && "$blob_base64" in value;

  if (editing) {
    return (
      <td>
        <input
          ref={inputRef}
          className={`cell-input ${invalid ? "invalid" : ""}`}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            try {
              coerceFromString(
                e.target.value,
                column.decl_type,
                !column.not_null,
              );
              setInvalid(false);
            } catch {
              setInvalid(true);
            }
          }}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              commit();
            } else if (e.key === "Escape") {
              e.preventDefault();
              cancel();
            }
          }}
        />
      </td>
    );
  }

  return (
    <td
      className={
        [
          editable ? "cell--editable" : "cell--locked",
          isNull ? "cell--null" : "",
          isBlob ? "cell--blob" : "",
          fk ? "cell--fk" : "",
        ]
          .filter(Boolean)
          .join(" ")
      }
      onDoubleClick={begin}
      title={
        fk
          ? `FK → ${fk.table}.${fk.to} (click ↗ to follow)`
          : editable
            ? "Double-click to edit"
            : "Locked"
      }
    >
      {fk && value !== null && (
        <button
          className="cell-fk-link"
          onClick={(e) => {
            e.stopPropagation();
            onFollowFk?.(fk);
          }}
          title={`Open ${fk.table}`}
        >
          ↗
        </button>
      )}
      <span className="cell-value">{formatValue(value)}</span>
    </td>
  );
}

function initialDraft(v: Value): string {
  if (v === null) return "";
  if (typeof v === "object" && v && "$blob_base64" in v) return "";
  return String(v);
}

function quote(s: string) {
  return '"' + s.replace(/"/g, '""') + '"';
}
