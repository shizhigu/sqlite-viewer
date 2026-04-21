import { foldKeymap } from "@codemirror/language";
import { sql, SQLite } from "@codemirror/lang-sql";
import { keymap } from "@codemirror/view";
import { githubLight, githubDark } from "@uiw/codemirror-theme-github";
import CodeMirror from "@uiw/react-codemirror";
import { useEffect, useMemo, useRef, useState } from "react";

import { append as recordHistory } from "../lib/history";
import { cmSchemaFromMap } from "../lib/loadSchemas";
import { sqlFold } from "../lib/sqlFold";
import type { AppError, QueryResult, Value } from "../lib/tauri";
import { tauri } from "../lib/tauri";
import { useAppStore } from "../store/app";

import { HistoryPalette } from "./HistoryPalette";

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
  const lastTokenRef = useRef<number | null>(null);
  const dark = useTheme();

  // ⌘P opens the history palette.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && (e.key === "p" || e.key === "P")) {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // When a pushed query arrives from `sqlv push`, mirror it into the editor
  // and show the result. Dedup by token so each push triggers exactly once.
  useEffect(() => {
    if (!pushedQuery) return;
    if (lastTokenRef.current === pushedQuery.token) return;
    lastTokenRef.current = pushedQuery.token;
    setText(pushedQuery.sql);
    setResult(pushedQuery.result);
    setError(pushedQuery.error);
    setPushedBadge(true);
    const t = setTimeout(() => setPushedBadge(false), 1500);
    return () => clearTimeout(t);
  }, [pushedQuery]);

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
    setError(null);
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
          onClick={() => setPaletteOpen(true)}
          title="⌘P — open query history"
        >
          ⌘P History
        </button>
      </div>
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
            ],
            [schemasByName],
          )}
          onChange={setText}
          basicSetup={{ lineNumbers: true, foldGutter: true }}
        />
      </div>
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
        onPick={(entry) => setText(entry.sql)}
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
