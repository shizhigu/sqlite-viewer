import { useEffect } from "react";

/**
 * Global ⌘+ / ⌘- / ⌘0 font scale.
 *
 * Scales typography only — `:root { --font-scale: N }`. All font-size
 * tokens in `tokens.css` are wrapped in `calc(Npx * var(--font-scale))`,
 * so changing this one variable resizes every piece of text in the app,
 * including the CodeMirror editor (via `.cm-content` / `.cm-gutters` in
 * `app.css`).
 *
 * The document coordinate system stays untouched. That matters because
 * WebKit's `document.caretRangeFromPoint` — which CodeMirror uses for
 * mousedown hit-testing — doesn't compensate for CSS `zoom` or
 * ancestor transforms, and either will break click-to-position in the
 * editor. Don't reintroduce them.
 *
 * Icons, borders, padding and gaps intentionally stay pixel-stable.
 * That's the convention in every IDE I've seen (VS Code's
 * "editor.fontSize" doesn't inflate the chrome either): scale text,
 * don't bloat the UI.
 */
const STORAGE_KEY = "sqlv.font-scale";
const LEGACY_ZOOM_KEY = "sqlv.zoom"; // old bad-zoom key — auto-cleaned
const MIN = 0.8;
const MAX = 1.4;
const STEP = 0.1;
const EPSILON = 0.001;

function clamp(n: number): number {
  return Math.max(MIN, Math.min(MAX, Math.round(n * 100) / 100));
}

function readCurrent(): number {
  const raw = document.documentElement.style.getPropertyValue("--font-scale");
  if (!raw) return 1;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? n : 1;
}

function apply(next: number): void {
  const v = clamp(next);
  if (Math.abs(v - 1) < EPSILON) {
    document.documentElement.style.removeProperty("--font-scale");
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      /* ignore */
    }
  } else {
    document.documentElement.style.setProperty("--font-scale", String(v));
    try {
      localStorage.setItem(STORAGE_KEY, String(v));
    } catch {
      /* ignore */
    }
  }
}

/** One-time cleanup of the old CSS-`zoom`-based key. Harmless no-op
 *  on a fresh install. */
function healLegacyZoom(): void {
  try {
    if (localStorage.getItem(LEGACY_ZOOM_KEY) !== null) {
      localStorage.removeItem(LEGACY_ZOOM_KEY);
    }
  } catch {
    /* ignore */
  }
  if (document.documentElement.style.zoom) {
    document.documentElement.style.removeProperty("zoom");
  }
}

export function useZoomShortcuts(): void {
  useEffect(() => {
    healLegacyZoom();

    // Restore a persisted font-scale if present.
    try {
      const saved = Number(localStorage.getItem(STORAGE_KEY));
      if (
        Number.isFinite(saved) &&
        saved > 0 &&
        Math.abs(saved - 1) > EPSILON
      ) {
        apply(saved);
      }
    } catch {
      /* ignore */
    }

    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      // `=` unshifted, `+` shifted — both map to "increase".
      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        apply(readCurrent() + STEP);
      } else if (e.key === "-") {
        e.preventDefault();
        apply(readCurrent() - STEP);
      } else if (e.key === "0") {
        e.preventDefault();
        apply(1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
