import { useMemo, useState } from "react";

import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

export function Sidebar() {
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
    // Picking a table is a "show me the data" intent; if the user is in
    // Query or Schema, snap back to Browse so their click has a visible
    // result instead of silently updating state in an off-screen tab.
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
    </aside>
  );
}

function TreeGroup({
  label,
  count,
  open,
  onToggle,
  children,
}: {
  label: string;
  count: number;
  open: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  if (count === 0) return null;
  return (
    <div className={`tree-group ${open ? "tree-group--open" : ""}`}>
      <button className="tree-group__header" onClick={onToggle}>
        <span className="tree-group__caret">▸</span>
        <span>{label}</span>
        <span style={{ marginLeft: "auto" }}>{count}</span>
      </button>
      {open && children}
    </div>
  );
}
