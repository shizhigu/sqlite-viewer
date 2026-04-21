/**
 * Last-session persistence for the desktop app.
 *
 * We store a tiny JSON blob in localStorage that lets us reopen exactly
 * what the user had up when the app last closed: the DB file, whether
 * it was read-write, which tab they were on, and which table was in
 * the grid. Everything else (query text, staged edits, etc.) is
 * deliberately NOT persisted — those are transient or live in their
 * own stores (history, saved queries, activity.db).
 *
 * Failure modes are silent by design: an unreadable / malformed blob
 * means a cold start, and a persisted DB path that no longer exists
 * means we just drop the hydration attempt and let the user open
 * something new. Session data is a hint, never load-bearing.
 */
import type { TabKind } from "../store/app";

const KEY = "sqlv.session";

export interface PersistedSession {
  dbPath: string | null;
  readWrite: boolean;
  activeTab: TabKind;
  selectedTable: string | null;
}

export function loadSession(): Partial<PersistedSession> | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) {
      console.debug("[sqlv:session] load: no session stored");
      return null;
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    console.debug("[sqlv:session] load:", parsed);
    return parsed as Partial<PersistedSession>;
  } catch (e) {
    console.warn("[sqlv:session] load failed:", e);
    return null;
  }
}

export function saveSession(s: Partial<PersistedSession>): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
    console.debug("[sqlv:session] save:", s);
  } catch (e) {
    console.warn("[sqlv:session] save failed:", e);
  }
}

export function clearSession(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // ignore
  }
}
