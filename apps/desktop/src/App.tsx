import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { ActivityPanel } from "./components/ActivityPanel";
import { BrowsePane } from "./components/BrowsePane";
import { QueryPane } from "./components/QueryPane";
import { SchemaPane } from "./components/SchemaPane";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Tabs } from "./components/Tabs";
import { Toasts } from "./components/Toasts";
import { Toolbar } from "./components/Toolbar";
import { loadAllSchemas } from "./lib/loadSchemas";
import { loadSession, saveSession } from "./lib/session";
import { tauri } from "./lib/tauri";
import type { DbMeta, QueryResult } from "./lib/tauri";
import { useZoomShortcuts } from "./lib/zoom";
import { useAppStore } from "./store/app";

import "./styles/tokens.css";
import "./styles/app.css";

export default function App() {
  const meta = useAppStore((s) => s.meta);
  const readWrite = useAppStore((s) => s.readWrite);
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const theme = useAppStore((s) => s.theme);
  const setMeta = useAppStore((s) => s.setMeta);
  const setTables = useAppStore((s) => s.setTables);
  const setViews = useAppStore((s) => s.setViews);
  const setSelectedTable = useAppStore((s) => s.setSelectedTable);
  const setSelectedSchema = useAppStore((s) => s.setSelectedSchema);
  const setReadWrite = useAppStore((s) => s.setReadWrite);
  const setPushedQuery = useAppStore((s) => s.setPushedQuery);
  const pushToast = useAppStore((s) => s.pushToast);
  const pushError = useAppStore((s) => s.pushError);
  const appendActivity = useAppStore((s) => s.appendActivity);
  const toggleActivity = useAppStore((s) => s.toggleActivity);
  const setSchemasByName = useAppStore((s) => s.setSchemasByName);

  // ⌘+ / ⌘- / ⌘0 — zoom in/out/reset
  useZoomShortcuts();

  // Flag + ref to gate two effects against the hydration run:
  //   - hydratedRef stops the persistence effect from overwriting
  //     localStorage with `null` before we've had a chance to read it;
  //   - hydrationAttemptedRef guarantees the hydration effect runs only
  //     once even across StrictMode double-mounting in dev;
  //   - lastSavedRef caches what we most recently wrote to localStorage
  //     so the subscribe below can diff without relying on Zustand's
  //     prev-state callback shape.
  const hydratedRef = useRef(false);
  const hydrationAttemptedRef = useRef(false);
  const lastSavedRef = useRef<{
    dbPath: string | null;
    readWrite: boolean;
    activeTab: string;
    selectedTable: string | null;
  } | null>(null);

  // Restore the previous session on cold start: re-open the DB the
  // user had up, put them back on the same tab, and select the same
  // table if it still exists. Silent on any failure — missing files,
  // renamed tables, and parse errors all collapse to "cold start".
  useEffect(() => {
    if (hydrationAttemptedRef.current) return;
    hydrationAttemptedRef.current = true;

    const session = loadSession();
    if (!session) {
      hydratedRef.current = true;
      return;
    }

    let cancelled = false;
    (async () => {
      try {
        if (session.activeTab) setActiveTab(session.activeTab);

        if (session.dbPath) {
          const m = await tauri.openDb(session.dbPath, !session.readWrite);
          if (cancelled) return;
          setMeta(m);
          setReadWrite(!!session.readWrite);

          const [t, v] = await Promise.all([
            tauri.listTables(),
            tauri.listViews(),
          ]);
          if (cancelled) return;
          setTables(t);
          setViews(v);

          const schemas = await loadAllSchemas([
            ...t.map((x) => x.name),
            ...v.map((x) => x.name),
          ]);
          if (cancelled) return;
          setSchemasByName(schemas);

          if (
            session.selectedTable &&
            [...t, ...v].some((x) => x.name === session.selectedTable)
          ) {
            setSelectedTable(session.selectedTable);
            try {
              const s = await tauri.describeTable(session.selectedTable);
              if (!cancelled) setSelectedSchema(s);
            } catch {
              // Renamed / dropped since last session — swallow.
            }
          }
        }
      } catch (e) {
        console.warn("[sqlv:session] hydration failed:", e);
        // File moved, perms changed, schema drift — all non-fatal.
      } finally {
        // Prime the last-saved snapshot with whatever ended up in the
        // store, so the subscribe's diff doesn't immediately fire a
        // redundant save on the first post-hydration change.
        const s = useAppStore.getState();
        lastSavedRef.current = {
          dbPath: s.meta?.path ?? null,
          readWrite: s.readWrite,
          activeTab: s.activeTab,
          selectedTable: s.selectedTable,
        };
        hydratedRef.current = true;
        console.debug("[sqlv:session] hydration done:", lastSavedRef.current);
      }
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Persist the session slice on every relevant change. Diffs against
  // `lastSavedRef` (declared above) so we don't depend on Zustand's
  // prev-state callback shape, which varies between configurations.
  // Gated on hydratedRef so state settling DURING restoration doesn't
  // overwrite the localStorage we just read.
  useEffect(() => {
    const unsub = useAppStore.subscribe((state) => {
      if (!hydratedRef.current) return;
      const next = {
        dbPath: state.meta?.path ?? null,
        readWrite: state.readWrite,
        activeTab: state.activeTab,
        selectedTable: state.selectedTable,
      };
      const prev = lastSavedRef.current;
      if (
        prev &&
        prev.dbPath === next.dbPath &&
        prev.readWrite === next.readWrite &&
        prev.activeTab === next.activeTab &&
        prev.selectedTable === next.selectedTable
      ) {
        return;
      }
      lastSavedRef.current = next;
      saveSession(next);
    });
    return unsub;
  }, []);

  // Apply theme mode to the document root. `auto` removes the attribute so
  // the CSS `prefers-color-scheme` rules take over.
  useEffect(() => {
    if (theme === "auto") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }, [theme]);

  // Global keyboard shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === "1") {
        e.preventDefault();
        setActiveTab("browse");
      } else if (mod && e.key === "2") {
        e.preventDefault();
        setActiveTab("query");
      } else if (mod && e.key === "3") {
        e.preventDefault();
        setActiveTab("schema");
      } else if (mod && e.shiftKey && (e.key === "A" || e.key === "a")) {
        e.preventDefault();
        toggleActivity();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setActiveTab]);

  // Listen for external pushes from the local loopback server (triggered by
  // `sqlv push` / `sqlv push-open`). Keep the handlers in App so state flows
  // through the single store.
  //
  // Race note: `listen()` returns a Promise. If the effect is re-run (React
  // StrictMode double-mount in dev, Tauri HMR, hot reload) before that
  // Promise resolves, the cleanup callback runs with an empty `unsubs`
  // array and the eventual handler is never detached. Every re-mount then
  // adds another live listener — which is exactly how we ended up with
  // one `pushed-open` event producing 4 identical "Opened …" toasts.
  //
  // Fix: await the Promise synchronously-ish inside an async IIFE and
  // check a `cancelled` flag before registering. If cleanup beat the
  // resolution, unsub immediately on arrival.
  useEffect(() => {
    let cancelled = false;
    const unsubs: UnlistenFn[] = [];
    const adopt = (u: UnlistenFn) => {
      if (cancelled) {
        u();
        return;
      }
      unsubs.push(u);
    };

    listen<{
      sql: string;
      result: QueryResult | null;
      error: { code: string; message: string } | null;
      token: number;
      pending?: boolean;
      kind?: "read_only" | "mutating";
      plan?: { id: number; parent: number; detail: string }[];
      affects?: { table: string; count: number };
    }>("pushed-query", (e) => {
      setPushedQuery({
        sql: e.payload.sql,
        result: e.payload.result,
        error: e.payload.error,
        token: e.payload.token,
        pending: e.payload.pending,
        kind: e.payload.kind,
        plan: e.payload.plan,
        affects: e.payload.affects,
      });
      setActiveTab("query");
      appendActivity({
        kind: "query",
        sql: e.payload.sql,
        rows: e.payload.result?.rows.length,
        elapsed_ms: e.payload.result?.elapsed_ms,
        error: e.payload.error,
      });
      if (e.payload.error) {
        pushError(e.payload.error);
      } else if (e.payload.pending) {
        pushToast(
          "info",
          `Agent proposed a ${e.payload.kind === "mutating" ? "write" : "query"} — review and Run when ready.`,
        );
      } else {
        pushToast("success", `Pushed query ran in ${e.payload.result?.elapsed_ms ?? 0} ms`);
      }
    }).then(adopt);

    listen<{
      path: string;
      read_only: boolean;
      meta: DbMeta | null;
      error: { code: string; message: string } | null;
      token: number;
    }>("pushed-open", async (e) => {
      appendActivity({
        kind: "open",
        path: e.payload.path,
        error: e.payload.error,
      });
      if (e.payload.error || !e.payload.meta) {
        pushError(e.payload.error ?? "unknown error opening database");
        return;
      }
      setMeta(e.payload.meta);
      setReadWrite(!e.payload.read_only);
      try {
        const [t, v] = await Promise.all([tauri.listTables(), tauri.listViews()]);
        setTables(t);
        setViews(v);
        setSelectedTable(null);
        setSelectedSchema(null);
        const schemas = await loadAllSchemas([
          ...t.map((x) => x.name),
          ...v.map((x) => x.name),
        ]);
        setSchemasByName(schemas);
        pushToast("success", `Opened ${e.payload.meta.path}`);
      } catch (err) {
        pushError(err as string);
      }
    }).then(adopt);

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
    // Deps are all stable Zustand setters; re-running this effect would
    // leak a second pair of listeners which is exactly the bug we just
    // fixed. Run once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="app" data-mode={readWrite ? "rw" : "ro"}>
      <Toolbar />
      <div className="app__middle">
        <Sidebar />
        <section className="content">
          <Tabs />
          {!meta ? (
            <div className="empty-state">
              Open a <code>.sqlite</code> / <code>.db</code> file to begin.
              <br />
              <small style={{ opacity: 0.6 }}>
                (Use the toolbar "Open database…" button)
              </small>
            </div>
          ) : activeTab === "browse" ? (
            <BrowsePane />
          ) : activeTab === "query" ? (
            <QueryPane />
          ) : (
            <SchemaPane />
          )}
        </section>
      </div>
      <StatusBar />
      <Toasts />
      <ActivityPanel />
    </div>
  );
}
