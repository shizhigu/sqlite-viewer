import { useEffect, useMemo, useState } from "react";

import type { SchemaInfo, TableInfo } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

export function Sidebar() {
  const meta = useAppStore((s) => s.meta);
  const tables = useAppStore((s) => s.tables);
  const views = useAppStore((s) => s.views);
  const selected = useAppStore((s) => s.selectedTable);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const pushError = useAppStore((s) => s.pushError);
  const [filter, setFilter] = useState("");
  const [openTables, setOpenTables] = useState(true);
  const [openViews, setOpenViews] = useState(true);

  // Attached-DB discovery. `main` is always here; `temp` is filtered out
  // below (almost never has user data). Anything a user has ATTACHed shows
  // up as its own group with its own tables list.
  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [tablesBySchema, setTablesBySchema] = useState<Record<string, TableInfo[]>>({});
  const [openSchema, setOpenSchema] = useState<Record<string, boolean>>({});

  useEffect(() => {
    // Attached-DB enumeration only makes sense once a DB is open.
    if (!meta) {
      setSchemas([]);
      setTablesBySchema({});
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const list = await tauri.listSchemas();
        if (cancelled) return;
        const extras = list.filter((s) => s.name !== "main" && s.name !== "temp");
        setSchemas(extras);
        const byName: Record<string, TableInfo[]> = {};
        for (const s of extras) {
          try {
            byName[s.name] = await tauri.listTablesInSchema(s.name);
          } catch {
            byName[s.name] = [];
          }
        }
        if (!cancelled) setTablesBySchema(byName);
      } catch {
        // Attached DB introspection is a nice-to-have; silently degrade.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [meta?.path, tables.length]);

  const filteredTables = useMemo(
    () =>
      tables.filter((t) => t.name.toLowerCase().includes(filter.toLowerCase())),
    [tables, filter],
  );
  const filteredViews = useMemo(
    () =>
      views.filter((v) => v.name.toLowerCase().includes(filter.toLowerCase())),
    [views, filter],
  );

  const pickTable = async (name: string) => {
    setSelectedTable(name);
    setActiveTab("browse");
    try {
      const s = await tauri.describeTable(name);
      setSelectedSchema(s);
    } catch (e) {
      pushError(e as string);
      setSelectedSchema(null);
    }
  };

  return (
    <aside className="sidebar">
      <div className="sidebar__filter">
        <input
          placeholder="Filter… (press /)"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          aria-label="Filter tables"
        />
      </div>

      <TreeGroup
        label="Tables"
        count={tables.length}
        open={openTables}
        onToggle={() => setOpenTables((v) => !v)}
      >
        {filteredTables.map((t) => (
          <button
            key={t.name}
            className={`tree-row ${selected === t.name ? "tree-row--selected" : ""}`}
            onClick={() => pickTable(t.name)}
          >
            <span>{t.name}</span>
            {t.row_count !== null && (
              <span className="tree-row__count">{t.row_count.toLocaleString()}</span>
            )}
          </button>
        ))}
      </TreeGroup>

      <TreeGroup
        label="Views"
        count={views.length}
        open={openViews}
        onToggle={() => setOpenViews((v) => !v)}
      >
        {filteredViews.map((v) => (
          <button
            key={v.name}
            className={`tree-row ${selected === v.name ? "tree-row--selected" : ""}`}
            onClick={() => pickTable(v.name)}
          >
            <span>{v.name}</span>
          </button>
        ))}
      </TreeGroup>

      {schemas.map((s) => {
        const list = tablesBySchema[s.name] ?? [];
        const isOpen = openSchema[s.name] ?? true;
        return (
          <TreeGroup
            key={s.name}
            label={`@ ${s.name}`}
            count={list.length}
            open={isOpen}
            onToggle={() =>
              setOpenSchema((m) => ({ ...m, [s.name]: !isOpen }))
            }
            badge="attached"
          >
            {list
              .filter((t) => t.name.toLowerCase().includes(filter.toLowerCase()))
              .map((t) => (
                <button
                  key={`${s.name}.${t.name}`}
                  className="tree-row tree-row--attached"
                  onClick={() => pickTable(t.name)}
                  title={`${s.name}.${t.name}`}
                >
                  <span>{t.name}</span>
                  {t.row_count !== null && (
                    <span className="tree-row__count">
                      {t.row_count.toLocaleString()}
                    </span>
                  )}
                </button>
              ))}
          </TreeGroup>
        );
      })}
    </aside>
  );
}

function TreeGroup({
  label,
  count,
  open,
  onToggle,
  children,
  badge,
}: {
  label: string;
  count: number;
  open: boolean;
  onToggle: () => void;
  children: React.ReactNode;
  badge?: string;
}) {
  if (count === 0) return null;
  return (
    <div className={`tree-group ${open ? "tree-group--open" : ""}`}>
      <button className="tree-group__header" onClick={onToggle}>
        <span className="tree-group__caret">▸</span>
        <span>{label}</span>
        {badge && <span className="tree-group__badge">{badge}</span>}
        <span style={{ marginLeft: "auto" }}>{count}</span>
      </button>
      {open && children}
    </div>
  );
}
