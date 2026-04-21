import type { TableSchema } from "./tauri";
import { tauri } from "./tauri";

/**
 * Fetch every table's schema via `describeTable` in parallel and return a
 * name → TableSchema map. This drives SQL completion and the schema pane.
 *
 * N+1 queries — acceptable for <100 tables (the catalogue sample has 9).
 * If we ever need scale, push a single `all_schemas()` into sqlv-core.
 */
export async function loadAllSchemas(
  tableNames: string[],
): Promise<Record<string, TableSchema>> {
  const results = await Promise.all(
    tableNames.map(async (name) => {
      try {
        return [name, await tauri.describeTable(name)] as const;
      } catch {
        return [name, null] as const;
      }
    }),
  );
  const out: Record<string, TableSchema> = {};
  for (const [name, schema] of results) {
    if (schema) out[name] = schema;
  }
  return out;
}

/** Reshape a TableSchema map into CodeMirror's completion schema shape. */
export function cmSchemaFromMap(
  map: Record<string, TableSchema>,
): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const [name, schema] of Object.entries(map)) {
    out[name] = schema.columns.map((c) => c.name);
  }
  return out;
}
