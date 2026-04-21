// Typed wrappers around `invoke(...)`. The shapes here mirror
// `sqlv_core::*` serde types — keep them in sync when the Rust side changes.

import { invoke } from "@tauri-apps/api/core";

// -------- types mirrored from sqlv-core --------

export interface DbMeta {
  path: string;
  size_bytes: number;
  page_size: number;
  page_count: number;
  encoding: string;
  user_version: number;
  application_id: number;
  journal_mode: string;
  sqlite_library_version: string;
  read_only: boolean;
}

export type TableKind = "table" | "view";

export interface TableInfo {
  name: string;
  kind: TableKind;
  row_count: number | null;
  sql: string | null;
}

export interface ViewInfo {
  name: string;
  sql: string | null;
}

export interface Column {
  cid: number;
  name: string;
  decl_type: string | null;
  not_null: boolean;
  default_value: string | null;
  pk: number;
  /** `PRAGMA table_xinfo` hidden flag. 0=normal, 1=virtual-table hidden,
   *  2=VIRTUAL generated, 3=STORED generated. Inline edit is only valid
   *  when this is 0. */
  hidden: number;
}

export interface ForeignKey {
  id: number;
  seq: number;
  table: string;
  from: string;
  to: string;
  on_update: string;
  on_delete: string;
  match: string;
}

export interface IndexInfo {
  name: string;
  table: string;
  unique: boolean;
  origin: string;
  partial: boolean;
  columns: string[];
}

export interface TableSchema {
  name: string;
  kind: TableKind;
  columns: Column[];
  foreign_keys: ForeignKey[];
  indexes: IndexInfo[];
  sql: string | null;
}

/**
 * Cell value mirrored from `sqlv-core::Value`. Tagged variants exist for
 * values JSON can't represent losslessly (i64 > 2^53, non-finite f64,
 * large blobs). See `crates/core/src/value.rs`.
 */
export type Value =
  | null
  | number
  | string
  | { $blob_base64: string }
  | { $blob_base64_truncated: string; $blob_size: number }
  | { $int64: string }
  | { $real: "NaN" | "Infinity" | "-Infinity" };

export interface QueryResult {
  columns: string[];
  column_types: (string | null)[];
  rows: Value[][];
  truncated: boolean;
  elapsed_ms: number;
}

export interface ExecResult {
  rows_affected: number;
  last_insert_rowid: number;
  elapsed_ms: number;
}

export interface AppError {
  code: string;
  message: string;
}

// -------- invoke wrappers --------

export const tauri = {
  ping: () => invoke<string>("ping"),
  openDb: (path: string, readOnly: boolean) =>
    invoke<DbMeta>("open_db", { path, readOnly }),
  listTables: () => invoke<TableInfo[]>("list_tables"),
  listViews: () => invoke<ViewInfo[]>("list_views"),
  describeTable: (name: string) =>
    invoke<TableSchema>("describe_table", { name }),
  runQuery: (sql: string, params: Value[], limit: number, offset: number) =>
    invoke<QueryResult>("run_query", { sql, params, limit, offset }),
  runExec: (sql: string, params: Value[]) =>
    invoke<ExecResult>("run_exec", { sql, params }),
  runExecMany: (statements: [string, Value[]][]) =>
    invoke<ExecResult>("run_exec_many", { statements }),
  closeDb: () => invoke<void>("close_db"),
};
