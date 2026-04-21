// Query history, per-DB, persisted to localStorage.
// Key: `sqlv.history.<db-path>` → JSON array of {sql, ts, elapsed_ms?, rows?}
// Cap: 100 most-recent per DB.

export interface HistoryEntry {
  sql: string;
  ts: number;
  elapsed_ms?: number;
  rows?: number;
  error?: boolean;
}

const CAP = 100;

function keyFor(dbPath: string): string {
  return `sqlv.history.${dbPath}`;
}

export function load(dbPath: string | null | undefined): HistoryEntry[] {
  if (!dbPath) return [];
  try {
    const raw = localStorage.getItem(keyFor(dbPath));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((e) => e && typeof e.sql === "string");
  } catch {
    return [];
  }
}

export function append(
  dbPath: string | null | undefined,
  entry: HistoryEntry,
): void {
  if (!dbPath) return;
  const current = load(dbPath);
  // Dedup against the immediately-previous entry with the same SQL — stops
  // ⌘⏎ hammering from filling the log.
  if (current.length > 0 && current[current.length - 1].sql === entry.sql) {
    current[current.length - 1] = entry;
  } else {
    current.push(entry);
  }
  const trimmed = current.slice(-CAP);
  try {
    localStorage.setItem(keyFor(dbPath), JSON.stringify(trimmed));
  } catch {
    // Storage quota — silently drop. History is best-effort.
  }
}

export function clear(dbPath: string | null | undefined): void {
  if (!dbPath) return;
  try {
    localStorage.removeItem(keyFor(dbPath));
  } catch {
    /* ignore */
  }
}
