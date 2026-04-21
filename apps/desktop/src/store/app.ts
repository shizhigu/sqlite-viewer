import { create } from "zustand";

import type {
  AppError,
  DbMeta,
  QueryResult,
  TableInfo,
  TableSchema,
  Value,
  ViewInfo,
} from "../lib/tauri";

export type TabKind = "browse" | "query" | "schema";
export type ThemeMode = "auto" | "light" | "dark";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
}

/** Incoming query from an external CLI invocation via `sqlv push`. */
export interface PushedQuery {
  sql: string;
  result: QueryResult | null;
  error: AppError | null;
  token: number;
  /** When `true`, the server did NOT execute — the UI should populate the
   * editor and show the "Agent proposed this — Run / Edit / Discard" bar. */
  pending?: boolean;
  /** Classifier verdict. `"mutating"` triggers the loud preview banner. */
  kind?: "read_only" | "mutating";
  /** EXPLAIN QUERY PLAN output, populated only for pending pushes. */
  plan?: { id: number; parent: number; detail: string }[];
  /** For UPDATE/DELETE pending pushes: rows the WHERE would touch. */
  affects?: { table: string; count: number };
}

/** One pending mutation accumulated while "Stage changes" is on. */
export interface StagedChange {
  id: number;
  table: string;
  op: "update" | "insert" | "delete";
  /** Ready-to-run SQL — the Commit button just hands all of these to
   *  `runExecMany` in insertion order, wrapped in one transaction. */
  sql: string;
  params: Value[];
  /** Human-readable one-line summary for the panel. */
  summary: string;
}

/** One entry in the agent-activity log (pushed query or pushed open). */
export interface ActivityEntry {
  id: number;
  ts: number;
  kind: "query" | "open";
  sql?: string;
  path?: string;
  rows?: number;
  elapsed_ms?: number;
  error?: { code: string; message: string } | null;
}

export interface AppStateShape {
  // Connection
  meta: DbMeta | null;
  readWrite: boolean; // mirrors the toolbar toggle; independent of OpenOpts
  tables: TableInfo[];
  views: ViewInfo[];
  selectedTable: string | null;
  selectedSchema: TableSchema | null;

  // Tabs
  activeTab: TabKind;

  // UI
  sidebarWidth: number;
  theme: ThemeMode;
  toasts: Toast[];

  // External-push state
  pushedQuery: PushedQuery | null;

  /** True while a query is in flight — drives the Cancel button in the
   *  toolbar. Set by QueryPane / DataGrid around their async operations. */
  queryRunning: boolean;

  /** When enabled, grid mutations (inline edits, add-row, delete-row) are
   *  enqueued into `stagedChanges` instead of hitting the DB immediately.
   *  Commit all at once via the Staged Changes panel. */
  stagingEnabled: boolean;
  stagedChanges: StagedChange[];

  // Activity log (drawer)
  activity: ActivityEntry[];
  activityOpen: boolean;

  // Full schema snapshot, keyed by table/view name — drives SQL completion.
  schemasByName: Record<string, TableSchema>;

  // Setters / actions
  setMeta: (m: DbMeta | null) => void;
  setTables: (t: TableInfo[]) => void;
  setViews: (v: ViewInfo[]) => void;
  setSelectedTable: (name: string | null) => void;
  setSelectedSchema: (s: TableSchema | null) => void;
  setActiveTab: (t: TabKind) => void;
  setReadWrite: (rw: boolean) => void;
  setSidebarWidth: (w: number) => void;
  setTheme: (t: ThemeMode) => void;
  setPushedQuery: (q: PushedQuery) => void;
  setQueryRunning: (running: boolean) => void;
  setStagingEnabled: (on: boolean) => void;
  addStagedChange: (c: Omit<StagedChange, "id">) => void;
  removeStagedChange: (id: number) => void;
  clearStagedChanges: () => void;
  appendActivity: (entry: Omit<ActivityEntry, "id" | "ts">) => void;
  toggleActivity: () => void;
  clearActivity: () => void;
  setSchemasByName: (m: Record<string, TableSchema>) => void;
  pushToast: (kind: Toast["kind"], text: string) => void;
  dismissToast: (id: number) => void;
  pushError: (e: AppError | string) => void;
  reset: () => void;
}

const THEME_KEY = "sqlv.theme";
function loadTheme(): ThemeMode {
  const v = localStorage.getItem(THEME_KEY);
  return v === "light" || v === "dark" || v === "auto" ? v : "auto";
}

let toastSeq = 1;

const defaults = {
  meta: null as DbMeta | null,
  readWrite: false,
  tables: [] as TableInfo[],
  views: [] as ViewInfo[],
  selectedTable: null as string | null,
  selectedSchema: null as TableSchema | null,
  activeTab: "browse" as TabKind,
  sidebarWidth: 260,
  toasts: [] as Toast[],
  pushedQuery: null as PushedQuery | null,
  queryRunning: false,
  stagingEnabled: false,
  stagedChanges: [] as StagedChange[],
  activity: [] as ActivityEntry[],
  activityOpen: false,
  schemasByName: {} as Record<string, TableSchema>,
};

let activitySeq = 1;
let stagedSeq = 1;

export const useAppStore = create<AppStateShape>((set) => ({
  ...defaults,
  theme: loadTheme(),

  setMeta: (meta) => set({ meta }),
  setTables: (tables) => set({ tables }),
  setViews: (views) => set({ views }),
  setSelectedTable: (selectedTable) => set({ selectedTable }),
  setSelectedSchema: (selectedSchema) => set({ selectedSchema }),
  setActiveTab: (activeTab) => set({ activeTab }),
  setReadWrite: (readWrite) => set({ readWrite }),
  setSidebarWidth: (sidebarWidth) => set({ sidebarWidth }),
  setTheme: (theme) => {
    localStorage.setItem(THEME_KEY, theme);
    set({ theme });
  },
  setPushedQuery: (pushedQuery) => set({ pushedQuery }),
  setQueryRunning: (queryRunning) => set({ queryRunning }),
  setStagingEnabled: (stagingEnabled) => set({ stagingEnabled }),
  addStagedChange: (c) =>
    set((s) => ({
      stagedChanges: [...s.stagedChanges, { ...c, id: stagedSeq++ }],
    })),
  removeStagedChange: (id) =>
    set((s) => ({
      stagedChanges: s.stagedChanges.filter((c) => c.id !== id),
    })),
  clearStagedChanges: () => set({ stagedChanges: [] }),
  appendActivity: (entry) =>
    set((s) => ({
      activity: [
        ...s.activity.slice(-199),
        { id: activitySeq++, ts: Date.now(), ...entry },
      ],
    })),
  toggleActivity: () => set((s) => ({ activityOpen: !s.activityOpen })),
  clearActivity: () => set({ activity: [] }),
  setSchemasByName: (schemasByName) => set({ schemasByName }),

  pushToast: (kind, text) =>
    set((s) => ({
      toasts: [...s.toasts, { id: toastSeq++, kind, text }],
    })),
  dismissToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  pushError: (e) => {
    const text =
      typeof e === "string"
        ? e
        : `${e.code}: ${e.message}`;
    set((s) => ({
      toasts: [...s.toasts, { id: toastSeq++, kind: "error", text }],
    }));
  },

  reset: () => set({ ...defaults }),
}));
