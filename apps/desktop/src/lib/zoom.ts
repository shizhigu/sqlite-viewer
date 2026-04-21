import { useEffect } from "react";

/**
 * Disabled feature: global ⌘+ / ⌘- / ⌘0 zoom.
 *
 * We used to set `document.documentElement.style.zoom` here. Don't.
 * CSS `zoom` on a WKWebView ancestor breaks `document.caretRangeFromPoint`:
 * click events come in in the zoomed coord space but CodeMirror's
 * (contenteditable-based) hit-test reads layout rects in the unzoomed
 * space, so every mid-line click rounded to end-of-line. Hours spent
 * chasing this bug.
 *
 * This hook is kept so existing imports don't break; it does two
 * things now:
 *   1. Clears any `sqlv.zoom` that a previous build persisted, plus the
 *      style property on the HTML element, so users who had a non-1
 *      zoom saved get auto-healed on next boot.
 *   2. Swallows the ⌘+ / ⌘- / ⌘0 key combos so the WebView's own
 *      default zoom behavior (which would have the same bug) is a
 *      no-op rather than visually surprising.
 *
 * If we want font scaling later, implement it with a CSS variable
 * (`--font-scale`) and em-based sizing — NOT with `zoom`.
 */
export function useZoomShortcuts(): void {
  useEffect(() => {
    try {
      localStorage.removeItem("sqlv.zoom");
    } catch {
      // ignore (private mode, disabled storage)
    }
    if (document.documentElement.style.zoom) {
      document.documentElement.style.removeProperty("zoom");
    }

    const swallow = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      if (e.key === "=" || e.key === "+" || e.key === "-" || e.key === "0") {
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", swallow);
    return () => window.removeEventListener("keydown", swallow);
  }, []);
}
