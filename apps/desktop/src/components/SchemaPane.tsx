import { useEffect, useState } from "react";

import type { TriggerInfo } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

/**
 * The Schema pane has two modes:
 *
 *  1. **Overview** (no table selected): list every table in the DB with
 *     row count, column count, FK/index/trigger counts. The user clicks
 *     a row to drill into the single-table view — same effect as
 *     picking from the sidebar, just reachable from within the tab.
 *     Before this existed, the tab was empty until a sidebar selection,
 *     and people thought the feature was missing.
 *
 *  2. **Detail** (a table is selected): columns / FKs / indexes /
 *     triggers / CREATE statement for that one table.
 */
export function SchemaPane() {
  const meta = useAppStore((s) => s.meta);
  const tables = useAppStore((s) => s.tables);
  const views = useAppStore((s) => s.views);
  const selectedSchema = useAppStore((s) => s.selectedSchema);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const schemasByName = useAppStore((s) => s.schemasByName);
  const pushError = useAppStore((s) => s.pushError);

  // Triggers — fetched once per open-DB since they're tiny and don't
  // change often.
  const [triggers, setTriggers] = useState<TriggerInfo[]>([]);
  useEffect(() => {
    if (!meta) return;
    tauri
      .listTriggers()
      .then(setTriggers)
      .catch(() => setTriggers([]));
  }, [meta?.path]);

  if (!meta) {
    return (
      <div className="empty-state">
        Open a database to inspect its schema.
      </div>
    );
  }

  const pickTable = async (name: string) => {
    setSelectedTable(name);
    try {
      const s = await tauri.describeTable(name);
      setSelectedSchema(s);
    } catch (e) {
      pushError(e as string);
    }
  };

  if (!selectedSchema) {
    return (
      <OverviewMode
        tables={tables}
        views={views}
        triggers={triggers}
        schemasByName={schemasByName}
        onPick={pickTable}
      />
    );
  }

  const tableTriggers = triggers.filter((t) => t.table === selectedSchema.name);

  return (
    <div className="schema-pane">
      <button
        className="btn schema-back"
        onClick={() => {
          setSelectedSchema(null);
          setSelectedTable(null);
        }}
        title="Back to all-tables overview"
      >
        ← All tables
      </button>

      <section className="schema-section">
        <h3>{selectedSchema.name}</h3>
        <div style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
          {selectedSchema.kind.toUpperCase()}
        </div>
        {selectedSchema.sql && (
          <details style={{ marginTop: "var(--s-2)" }}>
            <summary style={{ cursor: "pointer" }}>CREATE statement</summary>
            <pre className="mono" style={{ whiteSpace: "pre-wrap", marginTop: 8 }}>
              {selectedSchema.sql};
            </pre>
          </details>
        )}
      </section>

      <section className="schema-section">
        <h3>Columns</h3>
        <table>
          <thead>
            <tr>
              <th style={{ width: 30 }}>#</th>
              <th>Name</th>
              <th>Type</th>
              <th>Null</th>
              <th>Default</th>
              <th>PK</th>
              <th>Hidden</th>
            </tr>
          </thead>
          <tbody>
            {selectedSchema.columns.map((c) => (
              <tr key={c.cid}>
                <td>{c.cid}</td>
                <td>{c.name}</td>
                <td>{c.decl_type ?? ""}</td>
                <td>{c.not_null ? "NOT NULL" : ""}</td>
                <td>{c.default_value ?? ""}</td>
                <td>{c.pk > 0 ? `⚷ ${c.pk}` : ""}</td>
                <td>{renderHidden(c.hidden)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {selectedSchema.foreign_keys.length > 0 && (
        <section className="schema-section">
          <h3>Foreign keys</h3>
          <table>
            <thead>
              <tr>
                <th>From</th>
                <th>Target</th>
                <th>ON UPDATE</th>
                <th>ON DELETE</th>
              </tr>
            </thead>
            <tbody>
              {selectedSchema.foreign_keys.map((fk) => (
                <tr key={`${fk.id}-${fk.seq}`}>
                  <td>{fk.from}</td>
                  <td>
                    {fk.table}.{fk.to}
                  </td>
                  <td>{fk.on_update}</td>
                  <td>{fk.on_delete}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {selectedSchema.indexes.length > 0 && (
        <section className="schema-section">
          <h3>Indexes</h3>
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Columns</th>
                <th>Unique</th>
                <th>Origin</th>
              </tr>
            </thead>
            <tbody>
              {selectedSchema.indexes.map((i) => (
                <tr key={i.name}>
                  <td>{i.name}</td>
                  <td>{i.columns.join(", ")}</td>
                  <td>{i.unique ? "✓" : ""}</td>
                  <td>{i.origin}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {tableTriggers.length > 0 && (
        <section className="schema-section">
          <h3>Triggers</h3>
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Definition</th>
              </tr>
            </thead>
            <tbody>
              {tableTriggers.map((t) => (
                <tr key={t.name}>
                  <td>{t.name}</td>
                  <td>
                    <pre
                      className="mono"
                      style={{ whiteSpace: "pre-wrap", margin: 0 }}
                    >
                      {t.sql ?? "(no SQL)"}
                    </pre>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </div>
  );
}

/** All-tables overview. Shown when no single table is selected. */
function OverviewMode({
  tables,
  views,
  triggers,
  schemasByName,
  onPick,
}: {
  tables: { name: string; row_count: number | null }[];
  views: { name: string }[];
  triggers: TriggerInfo[];
  schemasByName: Record<string, { columns: unknown[]; foreign_keys: unknown[]; indexes: unknown[] }>;
  onPick: (name: string) => void;
}) {
  const triggerCountByTable = triggers.reduce<Record<string, number>>((acc, t) => {
    acc[t.table] = (acc[t.table] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="schema-pane">
      <section className="schema-section">
        <h3>All tables</h3>
        <p style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
          Click a row to inspect its columns, keys, indexes, and triggers.
        </p>
        <table className="schema-overview">
          <thead>
            <tr>
              <th>Table</th>
              <th style={{ textAlign: "right" }}>Rows</th>
              <th style={{ textAlign: "right" }}>Cols</th>
              <th style={{ textAlign: "right" }}>FKs</th>
              <th style={{ textAlign: "right" }}>Idx</th>
              <th style={{ textAlign: "right" }}>Trg</th>
            </tr>
          </thead>
          <tbody>
            {tables.map((t) => {
              const s = schemasByName[t.name];
              return (
                <tr key={t.name} onClick={() => onPick(t.name)}>
                  <td className="mono">{t.name}</td>
                  <td style={{ textAlign: "right" }}>
                    {t.row_count?.toLocaleString() ?? "—"}
                  </td>
                  <td style={{ textAlign: "right" }}>{s?.columns.length ?? "—"}</td>
                  <td style={{ textAlign: "right" }}>
                    {s?.foreign_keys.length ?? "—"}
                  </td>
                  <td style={{ textAlign: "right" }}>{s?.indexes.length ?? "—"}</td>
                  <td style={{ textAlign: "right" }}>
                    {triggerCountByTable[t.name] ?? 0}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </section>

      {views.length > 0 && (
        <section className="schema-section">
          <h3>Views</h3>
          <table className="schema-overview">
            <thead>
              <tr>
                <th>View</th>
                <th style={{ textAlign: "right" }}>Cols</th>
              </tr>
            </thead>
            <tbody>
              {views.map((v) => {
                const s = schemasByName[v.name];
                return (
                  <tr key={v.name} onClick={() => onPick(v.name)}>
                    <td className="mono">{v.name}</td>
                    <td style={{ textAlign: "right" }}>{s?.columns.length ?? "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>
      )}

      {triggers.length > 0 && (
        <section className="schema-section">
          <h3>All triggers</h3>
          <table className="schema-overview">
            <thead>
              <tr>
                <th>Name</th>
                <th>On table</th>
              </tr>
            </thead>
            <tbody>
              {triggers.map((t) => (
                <tr key={t.name} onClick={() => onPick(t.table)}>
                  <td className="mono">{t.name}</td>
                  <td className="mono">{t.table}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </div>
  );
}

/** `hidden` column from `PRAGMA table_xinfo`. 0 = normal, 1 = virtual-
 *  table hidden, 2 = VIRTUAL generated, 3 = STORED generated. */
function renderHidden(hidden: number): string {
  switch (hidden) {
    case 1:
      return "virtual-tbl";
    case 2:
      return "GENERATED VIRTUAL";
    case 3:
      return "GENERATED STORED";
    default:
      return "";
  }
}
