import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState } from "react";

import {
  buildDelete,
  buildInsert,
  buildUpdate,
  coerceFromString,
  formatValue,
  isEditableValueShape,
} from "../lib/sql";
import type { Column, ForeignKey, QueryResult, TableSchema, Value } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

/**
 * Page size for data fetches. We load this many rows in one network round
 * trip; react-virtual keeps DOM node count O(visible_rows) regardless.
 */
const PAGE_SIZE = 1000;
/** Pixel height of one rendered row — keep in sync with app.css grid rules. */
const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 36;

export interface DataGridProps {
  schema: TableSchema;
  totalRows: number | null;
  onMutated: () => void;
}

export function DataGrid({ schema, totalRows, onMutated }: DataGridProps) {
  const readWrite = useAppStore((s) => s.readWrite);
  const pushError = useAppStore((s) => s.pushError);
  const pushToast = useAppStore((s) => s.pushToast);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const setQueryRunning = useAppStore((s) => s.setQueryRunning);
  const stagingEnabled = useAppStore((s) => s.stagingEnabled);
  const setStagingEnabled = useAppStore((s) => s.setStagingEnabled);
  const addStagedChange = useAppStore((s) => s.addStagedChange);
  const stagedCount = useAppStore((s) => s.stagedChanges.length);

  const [page, setPage] = useState(0);
  const [result, setResult] = useState<QueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  // react-virtual scroller. The parent ref is the scrollable wrapper; the
  // virtualizer tells us a virtual list of rows with absolute Y-offsets.
  const parentRef = useRef<HTMLDivElement | null>(null);
  const rowVirtualizer = useVirtualizer({
    count: result?.rows.length ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
  });

  const pkCols = useMemo(
    () => schema.columns.filter((c) => c.pk > 0).sort((a, b) => a.pk - b.pk),
    [schema],
  );

  const fkByColumn = useMemo(() => {
    const m: Record<string, ForeignKey> = {};
    for (const fk of schema.foreign_keys) m[fk.from] = fk;
    return m;
  }, [schema]);

  const followFk = async (fk: ForeignKey) => {
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
      setQueryRunning(true);
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
        setQueryRunning(false);
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
    const oldValue = row[result.columns.indexOf(col.name)];
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
    const pkSummary = Object.entries(pk)
      .map(([k, v]) => `${k}=${formatValue(v)}`)
      .join(", ");
    if (stagingEnabled) {
      addStagedChange({
        table: schema.name,
        op: "update",
        sql,
        params,
        summary: `${col.name} (${pkSummary}): ${formatValue(oldValue)} → ${formatValue(newValue)}`,
      });
      pushToast("info", `Staged · ${col.name}`);
      return;
    }
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
    const { sql, params } = buildInsert(schema.name, {});
    if (stagingEnabled) {
      addStagedChange({
        table: schema.name,
        op: "insert",
        sql,
        params,
        summary: "default row",
      });
      pushToast("info", "Staged · + row");
      return;
    }
    try {
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
    const statements: [string, Value[]][] = [];
    const summaries: { sql: string; params: Value[]; summary: string }[] = [];
    for (const idx of selected) {
      const row = result.rows[idx];
      const pk: Record<string, Value> = {};
      for (const pkCol of pkCols) {
        const i = result.columns.indexOf(pkCol.name);
        pk[pkCol.name] = row[i];
      }
      const { sql, params } = buildDelete(schema.name, pk);
      statements.push([sql, params]);
      const pkSummary = Object.entries(pk)
        .map(([k, v]) => `${k}=${formatValue(v)}`)
        .join(", ");
      summaries.push({ sql, params, summary: pkSummary });
    }
    if (stagingEnabled) {
      for (const s of summaries) {
        addStagedChange({
          table: schema.name,
          op: "delete",
          sql: s.sql,
          params: s.params,
          summary: s.summary,
        });
      }
      pushToast("info", `Staged · − ${statements.length} row(s)`);
      setSelected(new Set());
      return;
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
  const totalLabel =
    totalRows !== null ? totalRows.toLocaleString() : `≥ ${endRow}`;
  const maxPage =
    totalRows !== null
      ? Math.max(0, Math.ceil(totalRows / PAGE_SIZE) - 1)
      : Infinity;
  const atLastPage =
    totalRows !== null ? page >= maxPage : !result.truncated;

  const virtualItems = rowVirtualizer.getVirtualItems();
  const totalVirtual = rowVirtualizer.getTotalSize();

  return (
    <div className="grid grid--virtual">
      <div className="grid__scroll" ref={parentRef}>
        <div
          className="grid__virtual-head"
          style={{ height: HEADER_HEIGHT }}
          role="row"
        >
          <div className="grid__cell grid__cell--idx">#</div>
          {schema.columns.map((col) => (
            <div
              key={col.name}
              className="grid__cell grid__cell--head"
              role="columnheader"
            >
              <span>
                {col.pk > 0 && <span className="col-badge">⚷</span>}
                {col.name}
              </span>
              <span className="col-type">
                {col.decl_type ?? ""} {col.not_null ? "· NOT NULL" : ""}
              </span>
            </div>
          ))}
        </div>
        <div
          className="grid__virtual-body"
          style={{ height: totalVirtual, position: "relative" }}
        >
          {virtualItems.map((vi) => {
            const rowIdx = vi.index;
            const row = result.rows[rowIdx];
            return (
              <div
                key={rowIdx}
                className={`grid__row ${selected.has(rowIdx) ? "grid-row--selected" : ""}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  right: 0,
                  transform: `translateY(${vi.start}px)`,
                  height: vi.size,
                }}
                onClick={(e) =>
                  toggleSelected(rowIdx, e.shiftKey, e.metaKey, selected, setSelected)
                }
              >
                <div className="grid__cell grid__cell--idx">
                  {page * PAGE_SIZE + rowIdx + 1}
                </div>
                {schema.columns.map((col, i) => (
                  <Cell
                    key={col.name}
                    value={row[i]}
                    column={col}
                    editable={
                      readWrite &&
                      col.pk === 0 &&
                      col.hidden === 0 &&
                      isEditableValueShape(row[i])
                    }
                    fk={fkByColumn[col.name]}
                    onFollowFk={(fk) => followFk(fk)}
                    onCommit={(raw) => commitCell(rowIdx, col, raw)}
                  />
                ))}
              </div>
            );
          })}
        </div>
      </div>
      <div className="grid__footer">
        {selected.size > 0 && <span>{selected.size} selected</span>}
        <label
          className="grid__stage-toggle"
          title="Queue every edit / add / delete locally instead of writing it immediately. Review the list up top, then hit Commit all to apply them in one transaction — succeed together or roll back together."
        >
          <input
            type="checkbox"
            checked={stagingEnabled}
            onChange={(e) => setStagingEnabled(e.target.checked)}
          />
          Batch edits
          {stagedCount > 0 && (
            <span className="chip staged__count-chip">{stagedCount}</span>
          )}
        </label>
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
      <div className="grid__cell">
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
      </div>
    );
  }

  return (
    <div
      className={[
        "grid__cell",
        editable ? "cell--editable" : "cell--locked",
        isNull ? "cell--null" : "",
        isBlob ? "cell--blob" : "",
        fk ? "cell--fk" : "",
      ]
        .filter(Boolean)
        .join(" ")}
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
    </div>
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
