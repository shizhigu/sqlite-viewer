import { foldKeymap } from "@codemirror/language";
import { sql, SQLite } from "@codemirror/lang-sql";
import { EditorView, keymap } from "@codemirror/view";
import { githubLight, githubDark } from "@uiw/codemirror-theme-github";
import CodeMirror from "@uiw/react-codemirror";
import { useEffect, useMemo, useRef, useState } from "react";

import { append as recordHistory } from "../lib/history";
import { cmSchemaFromMap } from "../lib/loadSchemas";
import { save as saveQuery } from "../lib/saved";
import { sqlFold } from "../lib/sqlFold";
import type { AppError, QueryResult, Value } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

import { HistoryPalette } from "./HistoryPalette";
import { SavedQueriesPalette } from "./SavedQueriesPalette";

export function QueryPane() {
  const meta = useAppStore((s) => s.meta);
  const pushedQuery = useAppStore((s) => s.pushedQuery);
  const schemasByName = useAppStore((s) => s.schemasByName);
  const [text, setText] = useState(
    "-- Press ⌘⏎ to run. ?1, ?2, … are positional params.\nSELECT 1 AS n;",
  );
  const [result, setResult] = useState<QueryResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [running, setRunning] = useState(false);
  const [params, setParams] = useState<string[]>([]);
  const [pushedBadge, setPushedBadge] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [savedPaletteOpen, setSavedPaletteOpen] = useState(false);
  const [pendingPush, setPendingPush] = useState<{
    sql: string;
    token: number;
  } | null>(null);
  // When an agent pushed a mutating query in `auto` mode, the server DOES
  // NOT execute it — instead we populate the editor and show a prominent
  // "Agent proposed this write" banner so the human approves explicitly.
  const [previewPending, setPreviewPending] = useState<{
    kind: "read_only" | "mutating";
    plan?: { id: number; parent: number; detail: string }[];
    affects?: { table: string; count: number };
  } | null>(null);
  // True whenever the editor text has diverged from "clean" state (a push
  // landed, or a replay, or — once the user starts typing — anything).
  const dirtyRef = useRef(false);
  const lastTokenRef = useRef<number | null>(null);
  const dark = useTheme();

  const pushToastAction = useAppStore((s) => s.pushToast);
  const setQueryRunning = useAppStore((s) => s.setQueryRunning);

  // Live row-count ghost — when the editor text looks like a simple
  // SELECT … FROM t [WHERE …], we probe COUNT(*) in the background and
  // render a subtle "≈ N rows" line. Debounced so we don't hammer the DB
  // while the user is typing.
  const [ghostCount, setGhostCount] = useState<{
    table: string;
    whereClause: string | null;
    count: number;
  } | null>(null);
  useEffect(() => {
    if (!meta) {
      setGhostCount(null);
      return;
    }
    const parsed = parseSimpleSelect(text);
    if (!parsed) {
      setGhostCount(null);
      return;
    }
    // Only auto-probe parameterless queries — otherwise we'd need values.
    if (/\?\d+/.test(parsed.whereClause ?? "")) {
      setGhostCount(null);
      return;
    }
    let cancelled = false;
    const id = window.setTimeout(() => {
      tauri
        .countRows(parsed.table, parsed.whereClause, [])
        .then((n) => {
          if (!cancelled) {
            setGhostCount({
              table: parsed.table,
              whereClause: parsed.whereClause,
              count: n,
            });
          }
        })
        .catch(() => {
          if (!cancelled) setGhostCount(null);
        });
    }, 400);
    return () => {
      cancelled = true;
      window.clearTimeout(id);
    };
  }, [text, meta?.path]);

  const promptAndSave = () => {
    if (!meta) return;
    const name = window.prompt("Save this query as:", "");
    if (!name) return;
    saveQuery(meta.path, { name, sql: text });
    pushToastAction("success", `Saved "${name.trim()}"`);
  };

  // ⌘P opens history, ⌘S opens saved queries, ⌘⇧S saves current.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && (e.key === "p" || e.key === "P")) {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      } else if (mod && e.shiftKey && (e.key === "s" || e.key === "S")) {
        e.preventDefault();
        promptAndSave();
      } else if (mod && !e.shiftKey && (e.key === "s" || e.key === "S")) {
        e.preventDefault();
        setSavedPaletteOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [meta?.path, text]);

  // When a pushed query arrives from `sqlv push`, mirror it into the editor
  // and show the result. Dedup by token so each push triggers exactly once.
  //
  // Concurrency rule: if the user has unsaved edits in the editor, DO NOT
  // overwrite them. Instead stash the incoming push in `pendingPush`; the
  // pill above the editor lets the user accept/dismiss explicitly.
  useEffect(() => {
    if (!pushedQuery) return;
    if (lastTokenRef.current === pushedQuery.token) return;
    lastTokenRef.current = pushedQuery.token;

    if (dirtyRef.current) {
      setPendingPush({ sql: pushedQuery.sql, token: pushedQuery.token });
      return;
    }

    setText(pushedQuery.sql);
    dirtyRef.current = false; // pushed text is the new "clean" baseline

    if (pushedQuery.pending) {
      // Agent proposed a write. Wait for the human to click Run.
      setResult(null);
      setError(null);
      setPreviewPending({
        kind: pushedQuery.kind ?? "mutating",
        plan: pushedQuery.plan,
        affects: pushedQuery.affects,
      });
    } else {
      setResult(pushedQuery.result);
      setError(pushedQuery.error);
      setPreviewPending(null);
    }
    setPushedBadge(true);
    const t = setTimeout(() => setPushedBadge(false), 1500);
    return () => clearTimeout(t);
  }, [pushedQuery]);

  const acceptPendingPush = () => {
    if (!pendingPush) return;
    setText(pendingPush.sql);
    dirtyRef.current = false;
    setPendingPush(null);
    setPushedBadge(true);
    setTimeout(() => setPushedBadge(false), 1500);
  };

  const dismissPendingPush = () => setPendingPush(null);

  // Detect ?N placeholders and size the params array accordingly.
  const placeholderCount = useMemo(() => {
    const matches = text.match(/\?(\d+)/g) ?? [];
    return matches.reduce(
      (max, m) => Math.max(max, Number(m.slice(1))),
      0,
    );
  }, [text]);

  useEffect(() => {
    setParams((prev) => {
      const next = prev.slice(0, placeholderCount);
      while (next.length < placeholderCount) next.push("");
      return next;
    });
  }, [placeholderCount]);

  const run = async () => {
    if (!meta) return;
    setRunning(true);
    setQueryRunning(true);
    setError(null);
    // Running commits the current editor text — subsequent pushes may
    // replace it without the "1 pending" pill unless the user edits again.
    dirtyRef.current = false;
    // Approving a pending agent-write: clear the banner.
    setPreviewPending(null);
    try {
      const parsed: Value[] = params.map((raw, i) => {
        if (raw === "") return null;
        try {
          return JSON.parse(raw) as Value;
        } catch (e) {
          throw {
            code: "usage",
            message: `param ?${i + 1}: invalid JSON (${(e as Error).message})`,
          } satisfies AppError;
        }
      });
      const res = await tauri.runQuery(text, parsed, 1000, 0);
      setResult(res);
      recordHistory(meta?.path, {
        sql: text,
        ts: Date.now(),
        elapsed_ms: res.elapsed_ms,
        rows: res.rows.length,
      });
    } catch (e) {
      const err = e as AppError;
      setError(typeof err === "string" ? { code: "other", message: err } : err);
      setResult(null);
      recordHistory(meta?.path, { sql: text, ts: Date.now(), error: true });
    } finally {
      setRunning(false);
      setQueryRunning(false);
    }
  };

  return (
    <div className="query">
      <div className="query__toolbar">
        <button
          className="btn btn--primary"
          onClick={run}
          disabled={running || !meta}
          title="⌘⏎"
        >
          {running ? "Running…" : "Run"}
        </button>
        <span style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
          {placeholderCount} param{placeholderCount === 1 ? "" : "s"}
          {result && ` · ${result.elapsed_ms} ms`}
        </span>
        {pushedBadge && (
          <span
            className="chip"
            style={{
              background: "var(--accent)",
              color: "var(--text-inverse)",
              borderColor: "var(--accent)",
              fontWeight: 600,
            }}
          >
            ↓ pushed from CLI
          </span>
        )}
        <span style={{ flex: 1 }} />
        <button
          className="btn"
          onClick={promptAndSave}
          title="⌘⇧S — save this query"
          disabled={!meta}
        >
          Save
        </button>
        <button
          className="btn"
          onClick={() => setSavedPaletteOpen(true)}
          title="⌘S — open saved queries"
          disabled={!meta}
        >
          ⌘S Saved
        </button>
        <button
          className="btn"
          onClick={() => setPaletteOpen(true)}
          title="⌘P — query history"
          disabled={!meta}
        >
          ⌘P History
        </button>
      </div>
      {pendingPush && (
        <div className="query__pending" role="status">
          <span className="query__pending-dot" aria-hidden>
            ●
          </span>
          <span>
            <strong>1 pending from CLI</strong> —{" "}
            <code className="mono">{truncateForPill(pendingPush.sql)}</code>
          </span>
          <span style={{ flex: 1 }} />
          <button className="btn btn--primary" onClick={acceptPendingPush}>
            Swap in
          </button>
          <button className="btn" onClick={dismissPendingPush}>
            Dismiss
          </button>
        </div>
      )}
      {previewPending && !pendingPush && (
        <div
          className={`query__preview query__preview--${previewPending.kind}`}
          role="alert"
        >
          <div className="query__preview-row">
            <span className="query__preview-icon" aria-hidden>
              {previewPending.kind === "mutating" ? "⚠" : "↓"}
            </span>
            <span>
              <strong>
                Agent proposed a{" "}
                {previewPending.kind === "mutating" ? "write" : "query"}
              </strong>{" "}
              — review, then Run (⌘⏎), or edit first.
            </span>
            {previewPending.affects && (
              <span className="query__preview-affects">
                affects{" "}
                <strong>
                  {previewPending.affects.count.toLocaleString()}
                </strong>{" "}
                row
                {previewPending.affects.count === 1 ? "" : "s"} in{" "}
                <code className="mono">{previewPending.affects.table}</code>
              </span>
            )}
            <span style={{ flex: 1 }} />
            <button
              className="btn"
              onClick={() => {
                setPreviewPending(null);
                setText("");
                dirtyRef.current = false;
              }}
            >
              Discard
            </button>
          </div>
          {previewPending.plan && previewPending.plan.length > 0 && (
            <details className="query__preview-plan">
              <summary>
                query plan ({previewPending.plan.length} node
                {previewPending.plan.length === 1 ? "" : "s"})
              </summary>
              <pre className="mono">
                {previewPending.plan
                  .map(
                    (n) =>
                      "  ".repeat(depthOf(n, previewPending.plan!)) + n.detail,
                  )
                  .join("\n")}
              </pre>
            </details>
          )}
        </div>
      )}
      <div
        className="query__editor"
        onKeyDown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault();
            run();
          }
        }}
      >
        <CodeMirror
          value={text}
          height="100%"
          theme={dark ? githubDark : githubLight}
          extensions={useMemo(
            () => [
              sql({
                dialect: SQLite,
                schema: cmSchemaFromMap(schemasByName),
                upperCaseKeywords: true,
              }),
              sqlFold,
              keymap.of(foldKeymap),
              // Wrap long lines visually instead of opening a horizontal
              // scrollbar. The underlying text is unchanged — selection
              // and copy-paste behave as if the line is still one line.
              EditorView.lineWrapping,
            ],
            [schemasByName],
          )}
          onChange={(v) => {
            setText(v);
            dirtyRef.current = true;
          }}
          basicSetup={{ lineNumbers: true, foldGutter: true }}
        />
      </div>
      {ghostCount && (
        <div className="query__count-ghost" role="status" aria-live="polite">
          ≈ <strong>{ghostCount.count.toLocaleString()}</strong> row
          {ghostCount.count === 1 ? "" : "s"} match{" "}
          {ghostCount.whereClause ? "this WHERE" : "the full table"} ·{" "}
          <code className="mono">{ghostCount.table}</code>
        </div>
      )}
      {placeholderCount > 0 && (
        <div className="query__params">
          {params.map((p, i) => (
            <span key={i} className="query__param">
              <label>?{i + 1}</label>
              <input
                value={p}
                placeholder="JSON"
                onChange={(e) => {
                  const next = [...params];
                  next[i] = e.target.value;
                  setParams(next);
                }}
              />
            </span>
          ))}
        </div>
      )}
      <div className="query__results">
        {error ? (
          <div className="query__error">
            [{error.code}] {error.message}
          </div>
        ) : result ? (
          <ResultTable result={result} />
        ) : (
          <div className="empty-state">Run a query to see results.</div>
        )}
      </div>
      <HistoryPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onPick={(entry) => {
          setText(entry.sql);
          dirtyRef.current = false;
        }}
      />
      <SavedQueriesPalette
        open={savedPaletteOpen}
        onClose={() => setSavedPaletteOpen(false)}
        onPick={(entry) => {
          setText(entry.sql);
          dirtyRef.current = false;
        }}
      />
    </div>
  );
}

function ResultTable({ result }: { result: QueryResult }) {
  return (
    <div className="grid" style={{ height: "100%" }}>
      {result.truncated && (
        <div className="grid__banner">
          Truncated to {result.rows.length} rows — add LIMIT to refine.
        </div>
      )}
      <div className="grid__scroll">
        <table>
          <thead>
            <tr>
              {result.columns.map((c, i) => (
                <th key={i}>
                  {c}
                  {result.column_types[i] && (
                    <span className="col-type">{result.column_types[i]}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {result.rows.map((row, i) => (
              <tr key={i}>
                {row.map((v, j) => (
                  <td
                    key={j}
                    className={v === null ? "cell--null" : ""}
                    title={v === null ? "NULL" : undefined}
                  >
                    {formatResultValue(v)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="grid__footer">
        <span>{result.rows.length} rows</span>
        <span className="spacer" />
        <span>{result.elapsed_ms} ms</span>
      </div>
    </div>
  );
}

function formatResultValue(v: Value): string {
  if (v === null) return "NULL";
  if (typeof v === "object" && v && "$blob_base64" in v) return "<blob>";
  return String(v);
}

function truncateForPill(s: string, n = 80): string {
  const one = s.replace(/\s+/g, " ").trim();
  return one.length <= n ? one : one.slice(0, n - 1) + "…";
}

/** Depth of a plan node in the parent chain (root = 0). Caps at 8 to
 *  defend against malformed / cyclic input. SQLite's plans are ≤ 4 deep. */
function depthOf(
  node: { id: number; parent: number; detail: string },
  nodes: { id: number; parent: number; detail: string }[],
): number {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  let d = 0;
  let cur = node.parent;
  while (cur !== 0 && d < 8) {
    d++;
    const p = byId.get(cur);
    if (!p) break;
    cur = p.parent;
  }
  return d;
}

/**
 * Best-effort parser for simple `SELECT … FROM <table> [WHERE …]` shapes.
 * Returns `null` for anything we can't safely probe with COUNT(*).
 *
 * Deliberately limited: no JOINs, no subqueries, no set ops. Those all
 * either have ambiguous row counts (joins explode rows) or would require
 * a proper AST. The ghost is an affordance, not a contract.
 */
function parseSimpleSelect(
  sql: string,
): { table: string; whereClause: string | null } | null {
  const stripped = sql
    .replace(/--[^\n]*/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .trim()
    .replace(/;\s*$/, "");
  if (!stripped) return null;
  const upper = stripped.toUpperCase();
  if (!upper.startsWith("SELECT ")) return null;
  // Reject anything that joins or combines — ambiguous row semantics.
  if (/\b(JOIN|UNION|INTERSECT|EXCEPT)\b/.test(upper)) return null;
  const fromIdx = upper.indexOf(" FROM ");
  if (fromIdx < 0) return null;
  let rest = stripped.slice(fromIdx + 6).trim();
  if (rest.startsWith("(")) return null; // FROM (subquery)
  // Strip table name (possibly quoted).
  let table = "";
  if (rest.startsWith('"')) {
    const end = rest.indexOf('"', 1);
    if (end < 0) return null;
    table = rest.slice(1, end).replace(/""/g, '"');
    rest = rest.slice(end + 1);
  } else {
    const m = rest.match(/^([A-Za-z_][\w]*)/);
    if (!m) return null;
    table = m[1];
    rest = rest.slice(m[1].length);
  }
  rest = rest.trimStart();
  // Reject alias or schema-qualified forms.
  if (rest.startsWith(".") || /^[A-Za-z_]/.test(rest)) return null;

  const restUpper = rest.toUpperCase();
  let whereClause: string | null = null;
  const whereIdx = restUpper.indexOf("WHERE ");
  if (whereIdx >= 0) {
    whereClause = rest.slice(whereIdx + 6).trim();
    // Strip trailing clauses that COUNT(*) doesn't care about.
    const tail =
      /\b(ORDER\s+BY|GROUP\s+BY|HAVING|LIMIT|OFFSET|WINDOW)\b/i.exec(
        whereClause,
      );
    if (tail) {
      whereClause = whereClause.slice(0, tail.index).trim();
    }
    if (!whereClause) whereClause = null;
  }
  return { table, whereClause };
}

/**
 * Resolve whether to render with a dark CodeMirror theme based on the
 * store's theme mode + the OS preference when in auto.
 */
function useTheme(): boolean {
  const theme = useAppStore((s) => s.theme);
  const [systemDark, setSystemDark] = useState(() =>
    window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false,
  );
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", listener);
    return () => mq.removeEventListener("change", listener);
  }, []);
  if (theme === "dark") return true;
  if (theme === "light") return false;
  return systemDark;
}
