import { useEffect } from "react";

// CSS `zoom` is supported by both WebKit (macOS WKWebView) and Chromium.
// We persist the level so reloads keep the user's pick.
//
// Important: at zoom = 1 we *clear* the style rather than set it to "1".
// In some WebKit builds, any explicit `zoom` value (even 1.0) shifts the
// pointer-event coordinate space subtly, which breaks click-drag selection
// inside CodeMirror. Only touch the style when we're actually zooming.

const STORAGE_KEY = "sqlv.zoom";
const MIN = 0.5;
const MAX = 3;
const STEP = 0.1;
const EPSILON = 0.001;

function clamp(z: number): number {
  // 2-decimal precision keeps the persisted value tidy.
  return Math.max(MIN, Math.min(MAX, Math.round(z * 100) / 100));
}

function readCurrent(): number {
  const raw = document.documentElement.style.zoom;
  if (!raw) return 1;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? n : 1;
}

function apply(z: number): void {
  const next = clamp(z);
  if (Math.abs(next - 1) < EPSILON) {
    document.documentElement.style.removeProperty("zoom");
    localStorage.removeItem(STORAGE_KEY);
  } else {
    document.documentElement.style.zoom = String(next);
    localStorage.setItem(STORAGE_KEY, String(next));
  }
}

export function useZoomShortcuts(): void {
  useEffect(() => {
    // Restore persisted zoom (if any) on mount. If none was saved, skip —
    // don't write `zoom: 1` into the style just to "reset".
    const saved = Number(localStorage.getItem(STORAGE_KEY));
    if (Number.isFinite(saved) && saved > 0 && Math.abs(saved - 1) > EPSILON) {
      apply(saved);
    }

    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      // `=` is the unshifted key on US layouts; `+` is the shifted glyph.
      // Most users press ⌘+= for "zoom in", so accept both.
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
