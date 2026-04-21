import { useAppStore } from "../store/app";

export function SchemaPane() {
  const s = useAppStore((x) => x.selectedSchema);
  if (!s) {
    return (
      <div className="empty-state">
        Pick a table from the sidebar to see its schema.
      </div>
    );
  }
  return (
    <div className="schema-pane">
      <section className="schema-section">
        <h3>{s.name}</h3>
        <div style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
          {s.kind.toUpperCase()}
        </div>
        {s.sql && (
          <details style={{ marginTop: "var(--s-2)" }}>
            <summary style={{ cursor: "pointer" }}>CREATE statement</summary>
            <pre className="mono" style={{ whiteSpace: "pre-wrap", marginTop: 8 }}>
              {s.sql};
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
            </tr>
          </thead>
          <tbody>
            {s.columns.map((c) => (
              <tr key={c.cid}>
                <td>{c.cid}</td>
                <td>{c.name}</td>
                <td>{c.decl_type ?? ""}</td>
                <td>{c.not_null ? "NOT NULL" : ""}</td>
                <td>{c.default_value ?? ""}</td>
                <td>{c.pk > 0 ? `⚷ ${c.pk}` : ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {s.foreign_keys.length > 0 && (
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
              {s.foreign_keys.map((fk) => (
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

      {s.indexes.length > 0 && (
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
              {s.indexes.map((i) => (
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
    </div>
  );
}
