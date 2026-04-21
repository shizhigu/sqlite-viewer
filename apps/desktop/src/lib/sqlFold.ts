import { foldService } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";

/**
 * Minimal SQL folding: fold multi-line parenthesized blocks, starting at
 * any line whose non-whitespace tail ends with `(` and folding up to the
 * line before the matching `)`.
 *
 * Covers the common SQL shapes — CTEs (`WITH x AS (…)`), subqueries,
 * long column lists, `VALUES (...)` blocks. Doesn't try to fold keyword
 * regions (SELECT/FROM/JOIN) because people format those too variously
 * for one heuristic to win.
 *
 * We deliberately do this with a `foldService` instead of a Lezer
 * `foldNodeProp`: the SQL grammar node names vary by dialect pack, and
 * the paren heuristic is stable across all of them.
 */
export const sqlFold = foldService.of(
  (state: EditorState, from: number): { from: number; to: number } | null => {
    const line = state.doc.lineAt(from);
    const trimmed = line.text.replace(/\s+$/, "");
    if (!trimmed.endsWith("(")) return null;

    // Scan forward from the end of the opening line to find the matching `)`.
    // Bail out if the document ends first (unbalanced).
    let depth = 1;
    let pos = line.to;
    const docLen = state.doc.length;
    while (pos < docLen && depth > 0) {
      pos++;
      const ch = state.doc.sliceString(pos - 1, pos);
      if (ch === "(") depth++;
      else if (ch === ")") depth--;
    }
    if (depth !== 0) return null;

    const closeLine = state.doc.lineAt(pos - 1);
    // Need at least one full line between open and close for a fold to be
    // meaningful; single-line `(...)` stays expanded.
    if (closeLine.number <= line.number + 1) return null;

    return { from: line.to, to: closeLine.from - 1 };
  },
);
