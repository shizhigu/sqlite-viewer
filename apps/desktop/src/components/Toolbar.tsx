import { open } from "@tauri-apps/plugin-dialog";

import { tauri } from "../lib/tauri";
import { useAppStore, type ThemeMode } from "../store/app";

export function Toolbar() {
  const meta = useAppStore((s) => s.meta);
  const readWrite = useAppStore((s) => s.readWrite);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const toggleActivity = useAppStore((s) => s.toggleActivity);
  const setMeta = useAppStore((s) => s.setMeta);
  const setReadWrite = useAppStore((s) => s.setReadWrite);
  const setTables = useAppStore((s) => s.setTables);
  const setViews = useAppStore((s) => s.setViews);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const pushError = useAppStore((s) => s.pushError);
  const pushToast = useAppStore((s) => s.pushToast);
  const queryRunning = useAppStore((s) => s.queryRunning);

  const cycleTheme = () => {
    const order: ThemeMode[] = ["auto", "light", "dark"];
    const i = order.indexOf(theme);
    setTheme(order[(i + 1) % order.length]);
  };

  const openFile = async () => {
    const file = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "SQLite", extensions: ["sqlite", "sqlite3", "db"] }],
    });
    if (!file) return;
    const path = typeof file === "string" ? file : (file as { path: string }).path;
    try {
      const m = await tauri.openDb(path, !readWrite);
      setMeta(m);
      const [t, v] = await Promise.all([tauri.listTables(), tauri.listViews()]);
      setTables(t);
      setViews(v);
      setSelectedTable(null);
      setSelectedSchema(null);
      pushToast("success", `Opened ${m.path}`);
    } catch (e) {
      pushError(e as string);
    }
  };

  const toggleMode = async () => {
    if (!meta) return;
    const target = !readWrite;
    // Re-open with the new mode.
    try {
      const m = await tauri.openDb(meta.path, !target);
      setMeta(m);
      setReadWrite(target);
      pushToast(
        target ? "success" : "info",
        target ? "Read-write enabled" : "Read-only enabled",
      );
    } catch (e) {
      pushError(e as string);
    }
  };

  return (
    <header className="toolbar">
      <div className="toolbar__left">
        <button className="btn" onClick={openFile}>
          Open database…
        </button>
        {meta ? (
          <span className="chip" title={meta.path}>
            {truncatePath(meta.path)}
          </span>
        ) : (
          <span className="chip">No database open</span>
        )}
      </div>
      <div className="toolbar__right">
        {queryRunning && (
          <button
            className="btn btn--danger toolbar__cancel"
            onClick={() => {
              tauri.cancelQuery().catch(() => {});
              pushToast("info", "Cancel signal sent");
            }}
            title="Interrupt the currently running query"
          >
            ■ Cancel
          </button>
        )}
        <button
          className="chip"
          onClick={toggleActivity}
          title="⌘⇧A — toggle agent activity panel"
        >
          ⚡ Activity
        </button>
        <button
          className="chip"
          onClick={cycleTheme}
          title="Click to cycle theme (Auto / Light / Dark)"
        >
          {theme === "auto" ? "◐ Auto" : theme === "light" ? "☀ Light" : "☾ Dark"}
        </button>
        <button
          className={`chip ${readWrite ? "chip--mode-rw" : "chip--mode-ro"}`}
          disabled={!meta}
          onClick={toggleMode}
          title="⌘⇧W — toggle read-write mode"
        >
          {readWrite ? "READ-WRITE" : "READ-ONLY"}
        </button>
      </div>
    </header>
  );
}

function truncatePath(p: string, max = 48): string {
  if (p.length <= max) return p;
  return "…" + p.slice(-(max - 1));
}
