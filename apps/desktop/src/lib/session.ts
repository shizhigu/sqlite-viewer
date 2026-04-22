/**
 * Last-session persistence, backed by `~/.sqlv/session.json`.
 *
 * We used to use `localStorage`, but in `tauri dev` that got cleared
 * often enough (HMR reloads, webview cache, origin churn) that restart
 * would land on an empty window even though we'd clearly written state
 * before closing. A real file in a well-known path is boring and
 * observable — `cat ~/.sqlv/session.json` tells you exactly what's
 * stored. Writes go through a Tauri command so the frontend doesn't
 * need filesystem perms of its own.
 */
import { invoke } from "@tauri-apps/api/core";

import type { TabKind } from "../store/app";

export interface PersistedSession {
  dbPath: string | null;
  readWrite: boolean;
  activeTab: TabKind;
  selectedTable: string | null;
}

export async function loadSession(): Promise<Partial<PersistedSession> | null> {
  try {
    const v = await invoke<Partial<PersistedSession> | null>("session_read");
    console.log("[sqlv:session] load:", v);
    if (!v || typeof v !== "object") return null;
    return v;
  } catch (e) {
    console.warn("[sqlv:session] load failed:", e);
    return null;
  }
}

export async function saveSession(s: Partial<PersistedSession>): Promise<void> {
  try {
    await invoke("session_write", { payload: s });
    console.log("[sqlv:session] save:", s);
  } catch (e) {
    console.warn("[sqlv:session] save failed:", e);
  }
}

