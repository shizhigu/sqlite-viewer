import { useCallback } from "react";

import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

import { DataGrid } from "./DataGrid";
import { StagedChangesPanel } from "./StagedChangesPanel";

export function BrowsePane() {
  const selectedSchema = useAppStore((s) => s.selectedSchema);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const tables = useAppStore((s) => s.tables);
  const setTables = useAppStore((s) => s.setTables);
  const pushError = useAppStore((s) => s.pushError);

  const refetch = useCallback(async () => {
    if (!selectedSchema) return;
    try {
      const [s, t] = await Promise.all([
        tauri.describeTable(selectedSchema.name),
        tauri.listTables(),
      ]);
      setSelectedSchema(s);
      setTables(t);
    } catch (e) {
      pushError(e as string);
    }
  }, [selectedSchema, setSelectedSchema, setTables, pushError]);

  if (!selectedSchema) {
    return (
      <div className="empty-state">
        Pick a table from the sidebar to browse its rows.
      </div>
    );
  }

  const totalRows =
    tables.find((t) => t.name === selectedSchema.name)?.row_count ?? null;

  return (
    <div className="browse">
      <StagedChangesPanel onCommitted={refetch} />
      <DataGrid
        schema={selectedSchema}
        totalRows={totalRows}
        onMutated={refetch}
      />
    </div>
  );
}
