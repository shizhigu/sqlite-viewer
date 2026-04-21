import type { Value } from "./tauri";

/**
 * Quote a SQLite identifier with double-quotes, escaping embedded quotes.
 * Use everywhere a table/column name is spliced into a SQL string. Names
 * come from `sqlite_master` introspection, so they're real SQLite
 * identifiers, but we still quote defensively.
 */
export function quoteIdent(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}

export interface ColumnRef {
  name: string;
  declType?: string | null;
}

/**
 * Build a parameterized UPDATE for a single-row edit.
 *
 * Returns `{ sql, params }` where `params` maps positionally to `?1..?N`
 * in `sql`. `pk` are WHERE-clause bindings, `updates` are SET-clause values.
 */
export function buildUpdate(
  table: string,
  updates: Record<string, Value>,
  pk: Record<string, Value>,
): { sql: string; params: Value[] } {
  const setCols = Object.keys(updates);
  const pkCols = Object.keys(pk);
  if (setCols.length === 0) {
    throw new Error("buildUpdate: updates must include at least one column");
  }
  if (pkCols.length === 0) {
    throw new Error("buildUpdate: pk must include at least one column");
  }

  const params: Value[] = [];
  const setFrag = setCols
    .map((c) => {
      params.push(updates[c]);
      return `${quoteIdent(c)} = ?${params.length}`;
    })
    .join(", ");
  const whereFrag = pkCols
    .map((c) => {
      params.push(pk[c]);
      return `${quoteIdent(c)} = ?${params.length}`;
    })
    .join(" AND ");

  return {
    sql: `UPDATE ${quoteIdent(table)} SET ${setFrag} WHERE ${whereFrag}`,
    params,
  };
}

export function buildInsert(
  table: string,
  values: Record<string, Value>,
): { sql: string; params: Value[] } {
  const cols = Object.keys(values);
  if (cols.length === 0) {
    return {
      sql: `INSERT INTO ${quoteIdent(table)} DEFAULT VALUES`,
      params: [],
    };
  }
  const params: Value[] = cols.map((c) => values[c]);
  const colFrag = cols.map(quoteIdent).join(", ");
  const placeholders = cols.map((_, i) => `?${i + 1}`).join(", ");
  return {
    sql: `INSERT INTO ${quoteIdent(table)} (${colFrag}) VALUES (${placeholders})`,
    params,
  };
}

export function buildDelete(
  table: string,
  pk: Record<string, Value>,
): { sql: string; params: Value[] } {
  const cols = Object.keys(pk);
  if (cols.length === 0) {
    throw new Error("buildDelete: pk must include at least one column");
  }
  const params: Value[] = cols.map((c) => pk[c]);
  const whereFrag = cols
    .map((c, i) => `${quoteIdent(c)} = ?${i + 1}`)
    .join(" AND ");
  return {
    sql: `DELETE FROM ${quoteIdent(table)} WHERE ${whereFrag}`,
    params,
  };
}

/** Best-effort coercion from a string the user typed into a typed `Value`. */
export function coerceFromString(
  raw: string,
  declType: string | null | undefined,
  allowNullOnEmpty: boolean,
): Value {
  if (raw === "" && allowNullOnEmpty) return null;
  const t = (declType || "").toUpperCase();
  if (t.includes("INT")) {
    if (!/^-?\d+$/.test(raw)) {
      throw new Error("INTEGER cell must be a whole number");
    }
    return Number(raw);
  }
  if (t.includes("REAL") || t.includes("FLOAT") || t.includes("DOUB")) {
    const n = Number(raw);
    if (Number.isNaN(n)) throw new Error("REAL cell must be a number");
    return n;
  }
  return raw;
}

/** Render a `Value` into its cell display string. */
export function formatValue(v: Value): string {
  if (v === null) return "NULL";
  if (typeof v === "object" && v && "$blob_base64" in v) {
    const len = estimateBlobBytes((v as { $blob_base64: string }).$blob_base64);
    return `<blob · ${len}b>`;
  }
  return String(v);
}

function estimateBlobBytes(b64: string): number {
  // Three bytes per 4 base64 chars, minus padding.
  const padding = (b64.match(/=+$/)?.[0] ?? "").length;
  return Math.max(0, Math.floor((b64.length * 3) / 4) - padding);
}
