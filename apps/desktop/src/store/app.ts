import { create } from "zustand";

import type {
  AppError,
  DbMeta,
  QueryResult,
  TableInfo,
  TableSchema,
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
  activity: [] as ActivityEntry[],
  activityOpen: false,
  schemasByName: {} as Record<string, TableSchema>,
};

let activitySeq = 1;

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
