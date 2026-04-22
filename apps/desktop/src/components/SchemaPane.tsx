import { useEffect, useState } from "react";

import type { SchemaInfo, TableInfo, TriggerInfo } from "../lib/tauri";
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

  // Schema switcher — `main`, `temp` (if non-empty), plus anything the
  // user has ATTACHed. Defaults to `main`; changing it swaps which
  // schema's tables the overview shows. The store's `tables` / `views`
  // arrays are the main schema; other schemas get fetched lazily.
  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [activeSchema, setActiveSchema] = useState<string>("main");
  const [attachedTables, setAttachedTables] = useState<TableInfo[]>([]);
  const [loadingAttached, setLoadingAttached] = useState(false);
  useEffect(() => {
    if (!meta) {
      setSchemas([]);
      return;
    }
    tauri
      .listSchemas()
      .then((list) => setSchemas(list.filter((s) => s.name !== "temp")))
      .catch(() => setSchemas([]));
  }, [meta?.path]);

  useEffect(() => {
    if (activeSchema === "main") {
      setAttachedTables([]);
      return;
    }
    setLoadingAttached(true);
    tauri
      .listTablesInSchema(activeSchema)
      .then(setAttachedTables)
      .catch(() => setAttachedTables([]))
      .finally(() => setLoadingAttached(false));
  }, [activeSchema, meta?.path]);

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
    const tablesForActive = activeSchema === "main" ? tables : attachedTables;
    const viewsForActive = activeSchema === "main" ? views : [];
    const triggersForActive = activeSchema === "main" ? triggers : [];
    return (
      <OverviewMode
        schemas={schemas}
        activeSchema={activeSchema}
        onActiveSchemaChange={setActiveSchema}
        tables={tablesForActive}
        views={viewsForActive}
        triggers={triggersForActive}
        schemasByName={schemasByName}
        onPick={pickTable}
        loading={loadingAttached}
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
  schemas,
  activeSchema,
  onActiveSchemaChange,
  tables,
  views,
  triggers,
  schemasByName,
  onPick,
  loading,
}: {
  schemas: SchemaInfo[];
  activeSchema: string;
  onActiveSchemaChange: (name: string) => void;
  tables: { name: string; row_count: number | null }[];
  views: { name: string }[];
  triggers: TriggerInfo[];
  schemasByName: Record<string, { columns: unknown[]; foreign_keys: unknown[]; indexes: unknown[] }>;
  onPick: (name: string) => void;
  loading: boolean;
}) {
  const triggerCountByTable = triggers.reduce<Record<string, number>>((acc, t) => {
    acc[t.table] = (acc[t.table] ?? 0) + 1;
    return acc;
  }, {});

  // Only show the picker when there's more than one schema to pick
  // from. For 99% of DBs that don't use ATTACH, we keep the UI quiet.
  const showPicker = schemas.length > 1;

  return (
    <div className="schema-pane">
      {showPicker && (
        <div className="schema-picker" role="tablist" aria-label="Database schema">
          {schemas.map((s) => (
            <button
              key={s.name}
              role="tab"
              aria-selected={s.name === activeSchema}
              className={`schema-picker__tab ${
                s.name === activeSchema ? "schema-picker__tab--active" : ""
              }`}
              onClick={() => onActiveSchemaChange(s.name)}
              title={s.file || "(in-memory)"}
            >
              <span className="schema-picker__name">{s.name}</span>
              {s.name !== "main" && (
                <span className="schema-picker__badge">attached</span>
              )}
            </button>
          ))}
        </div>
      )}
      <section className="schema-section">
        <h3>
          Tables{" "}
          {activeSchema !== "main" && (
            <span className="schema-scope mono">· {activeSchema}</span>
          )}
        </h3>
        <p style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
          {loading
            ? "Loading…"
            : "Click a row to inspect its columns, keys, indexes, and triggers."}
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
