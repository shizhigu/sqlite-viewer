// Saved queries, per-DB, persisted to localStorage.
// Key: `sqlv.saved.<db-path>` → JSON array of {name, sql, ts}.
// Case-insensitive dedup by name — re-saving replaces the entry.
//
// Ideally these would live in a `.sqlvqueries.json` file next to the DB
// so they can be committed to git. That requires broader Tauri FS
// permissions; localStorage is the MVP.

export interface SavedQuery {
  name: string;
  sql: string;
  ts: number;
}

function keyFor(dbPath: string): string {
  return `sqlv.saved.${dbPath}`;
}

export function load(dbPath: string | null | undefined): SavedQuery[] {
  if (!dbPath) return [];
  try {
    const raw = localStorage.getItem(keyFor(dbPath));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e): e is SavedQuery =>
        !!e && typeof e.name === "string" && typeof e.sql === "string",
    );
  } catch {
    return [];
  }
}

export function save(
  dbPath: string | null | undefined,
  entry: Omit<SavedQuery, "ts">,
): SavedQuery[] {
  if (!dbPath) return [];
  const trimmed = { ...entry, name: entry.name.trim() };
  if (!trimmed.name) return load(dbPath);
  const current = load(dbPath);
  const next = current.filter(
    (e) => e.name.toLowerCase() !== trimmed.name.toLowerCase(),
  );
  next.push({ ...trimmed, ts: Date.now() });
  next.sort((a, b) => a.name.localeCompare(b.name));
  try {
    localStorage.setItem(keyFor(dbPath), JSON.stringify(next));
  } catch {
    /* storage quota — silently drop */
  }
  return next;
}

export function remove(
  dbPath: string | null | undefined,
  name: string,
): SavedQuery[] {
  if (!dbPath) return [];
  const next = load(dbPath).filter(
    (e) => e.name.toLowerCase() !== name.toLowerCase(),
  );
  try {
    localStorage.setItem(keyFor(dbPath), JSON.stringify(next));
  } catch {
    /* ignore */
  }
  return next;
}
