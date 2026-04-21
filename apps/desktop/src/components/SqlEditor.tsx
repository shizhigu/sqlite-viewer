import { sql, SQLite } from "@codemirror/lang-sql";
import { foldKeymap } from "@codemirror/language";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { githubDark, githubLight } from "@uiw/codemirror-theme-github";
import { basicSetup } from "codemirror";
import { useEffect, useRef } from "react";

import { cmSchemaFromMap } from "../lib/loadSchemas";
import { sqlFold } from "../lib/sqlFold";
import type { TableSchema } from "../lib/tauri";

/**
 * Thin, direct-init CodeMirror 6 wrapper.
 *
 * We used to use `@uiw/react-codemirror`; it kept breaking the click-to-
 * position hit-test (cursor snapped to end-of-line, no mid-line clicks).
 * The root cause class was the wrapper's mount lifecycle + `height="100%"`
 * proxy + StrictMode double-mount re-init — by the time EditorView
 * measured its geometry, the surrounding grid row wasn't settled, and
 * CodeMirror cached the wrong `contentRect` forever.
 *
 * Fix: own the lifecycle. Mount an EditorView synchronously into a plain
 * `<div>`, destroy it on unmount, hot-swap the two things that change at
 * runtime (theme, SQL schema for completion) through Compartments so we
 * never re-init the view.
 *
 * API kept intentionally tiny so QueryPane doesn't have to know anything
 * about CodeMirror internals.
 */
export interface SqlEditorProps {
  /** Current editor text. On mount we seed the doc with this; subsequent
   *  changes from outside (push, history pick, etc.) get dispatched as
   *  full-text replacements guarded by a string-equality check to avoid
   *  a feedback loop with our own `onChange`. */
  value: string;
  onChange: (v: string) => void;
  /** Fired when the user presses ⌘⏎ / Ctrl+Enter inside the editor. */
  onRun: () => void;
  /** Name → schema map, used to drive SQL completion. Swapping this at
   *  runtime reconfigures the `sql()` language extension via a
   *  Compartment — the view stays alive. */
  schemasByName: Record<string, TableSchema>;
  /** true for dark theme, false for light. Toggled live via a theme
   *  Compartment. */
  dark: boolean;
}

export function SqlEditor({
  value,
  onChange,
  onRun,
  schemasByName,
  dark,
}: SqlEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const themeCompartmentRef = useRef(new Compartment());
  const sqlCompartmentRef = useRef(new Compartment());

  // Stash the latest callbacks in refs so the init effect below can keep
  // `[]` deps without capturing stale closures when the parent rerenders.
  const onChangeRef = useRef(onChange);
  const onRunRef = useRef(onRun);
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  useEffect(() => {
    onRunRef.current = onRun;
  }, [onRun]);

  // ----- mount (and only mount) -----
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        basicSetup,
        sqlCompartmentRef.current.of(
          sql({
            dialect: SQLite,
            schema: cmSchemaFromMap(schemasByName),
            upperCaseKeywords: true,
          }),
        ),
        sqlFold,
        keymap.of([
          {
            key: "Mod-Enter",
            preventDefault: true,
            run: () => {
              onRunRef.current();
              return true;
            },
          },
          ...foldKeymap,
        ]),
        EditorView.lineWrapping,
        themeCompartmentRef.current.of(dark ? githubDark : githubLight),
        // Push doc changes back up. Guard on `docChanged` so selection-only
        // updates don't fire a redundant onChange.
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onChangeRef.current(u.state.doc.toString());
        }),
      ],
    });

    const view = new EditorView({ state, parent: host });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ----- runtime reconfig: theme -----
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: themeCompartmentRef.current.reconfigure(
        dark ? githubDark : githubLight,
      ),
    });
  }, [dark]);

  // ----- runtime reconfig: completion schema -----
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: sqlCompartmentRef.current.reconfigure(
        sql({
          dialect: SQLite,
          schema: cmSchemaFromMap(schemasByName),
          upperCaseKeywords: true,
        }),
      ),
    });
  }, [schemasByName]);

  // ----- runtime sync: external `value` -----
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
    });
  }, [value]);

  return (
    <div
      ref={hostRef}
      className="sql-editor"
      style={{ height: "100%", width: "100%" }}
    />
  );
}
