# DESIGN.md — SQLite Viewer desktop app

Authoritative UI spec. Implement components from this document — do not invent visual decisions inline. If the code diverges from the doc, update the doc first, then the code.

## 1. Principles

1. **Information-dense, never crowded.** This app is for people who want to see data. Prefer compact typography, tight line-height, and hairline dividers over generous whitespace.
2. **Keyboard-first.** Every routine action has a shortcut visible next to its label. Mouse-only flows are a last resort.
3. **Native-feeling on macOS.** System font (`-apple-system`), native traffic-light bar, window vibrancy off (we want crisp data, not blurred chrome), system-driven dark mode.
4. **Calm by default, loud on error.** Neutral greys dominate; accent color only for primary action and selection. Red, orange, and green are reserved for error / warning / success states respectively — never decoration. When these states fire they must be **unmissable**: filled red destructive buttons, filled red error toasts, a filled warning strip across the top of the window in read-write mode. Do not soften these.
5. **Restraint on animation.** Motion is a communication tool, not a flourish. Max 150 ms, ease-out, and only on state change (panel expand, toast enter, row commit).
6. **Write mode is a mode.** The app reads the database until the user explicitly flips a toggle. The toggle state must be legible at a glance in every view.

## 2. Visual tokens

### 2.1 Color palette

Defined as CSS custom properties. Two sets — `[data-theme="light"]` and `[data-theme="dark"]` — resolved from `prefers-color-scheme` by default, overrideable per session.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#fafafa` | `#1a1a1c` | Window background |
| `--bg-panel` | `#ffffff` | `#232327` | Sidebar, editor, grid |
| `--bg-row-alt` | `#f6f6f7` | `#2a2a2e` | Zebra stripe in data grid |
| `--bg-row-selected` | `#e7f0ff` | `#2a3a55` | Selected row |
| `--bg-row-editing` | `#fff7d6` | `#3a3520` | Row with pending edit |
| `--border` | `#e5e5e7` | `#3a3a3e` | Default hairline |
| `--border-strong` | `#c9c9cd` | `#4a4a4f` | Emphasized divider, input outline |
| `--text` | `#1a1a1c` | `#f2f2f4` | Default text |
| `--text-muted` | `#6b6b70` | `#9a9aa0` | Secondary text, column types |
| `--text-inverse` | `#ffffff` | `#0a0a0c` | Text on filled buttons |
| `--accent` | `#0a6cff` | `#3d8eff` | Primary action, selected tab |
| `--accent-weak` | `#cfe0ff` | `#1d355f` | Accent background fill |
| `--danger` | `#c5302b` | `#ff5e58` | Destructive action, SQL error |
| `--warning` | `#b05f00` | `#ffaa44` | Unsaved, truncated result, write mode |
| `--success` | `#1a7a3d` | `#4cd07a` | Success toast |
| `--focus-ring` | `#0a6cff33` | `#3d8eff55` | 2px focus outline |

### 2.2 Typography

- **UI:** `-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif`
- **Mono (code, cell values, SQL):** `'SF Mono', 'JetBrains Mono', Menlo, Consolas, monospace`
- Scale: `--text-xs: 11px / 15px`, `--text-sm: 12px / 17px`, `--text-base: 13px / 18px`, `--text-lg: 15px / 20px`, `--text-xl: 18px / 24px`
- Weights: 400 (body), 500 (labels, tab titles), 600 (heading)
- Default body: `--text-sm` 12px — denser than web norms, matches native Mac apps (TablePlus, Proxyman).

### 2.3 Spacing

4-based scale: `--s-1: 4px`, `--s-2: 8px`, `--s-3: 12px`, `--s-4: 16px`, `--s-5: 24px`, `--s-6: 32px`, `--s-7: 48px`. Gutters between grid cells: 0 (hairlines). Gutters between UI sections: `--s-3` or `--s-4`.

### 2.4 Radii, shadows, motion

- Radii: `--r-sm: 4px` (inputs, cells), `--r-md: 6px` (buttons, panels), `--r-lg: 10px` (modal, dialog). Tabs have **0 radius** on the bottom to butt into the content area.
- Shadows: `--shadow-sm: 0 1px 2px rgba(0,0,0,0.04)` (raised button), `--shadow-md: 0 6px 16px rgba(0,0,0,0.12)` (popover, dropdown). Light theme only; dark theme uses borders instead.
- Motion: `--ease: cubic-bezier(0.2, 0.8, 0.2, 1)`, `--dur-fast: 80ms`, `--dur: 150ms`. Animate: panel expand, toast slide, tab indicator. **Do not animate:** data-grid rows, query results, cell edits.

## 3. Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  Toolbar  [open file…]  [path: ~/app.sqlite]       [⛭ RW/RO]    │  40px
├──────────┬──────────────────────────────────────────────────────┤
│ Sidebar  │  Tabs: [Browse] [Query] [Schema]                     │  32px
│          ├──────────────────────────────────────────────────────┤
│ ▸ Tables │                                                      │
│   users  │                                                      │
│   orders │                  Tab content                         │
│ ▸ Views  │                                                      │
│ ▸ Indexes│                                                      │
│          │                                                      │
│ 260px    │                                                      │
├──────────┴──────────────────────────────────────────────────────┤
│  StatusBar  v3.47.0 • UTF-8 • 4096 pg • 12ms  [READ-ONLY]       │  24px
└─────────────────────────────────────────────────────────────────┘
```

- Minimum window: 880×560.
- Sidebar: resizable, 220–420 px. Persisted per-session in Zustand.
- Sidebar collapses below 140 px → icon-rail mode (v2; show a placeholder in v1).
- Status bar is always visible, 24 px, monospace right-side metrics.

## 4. Components

### 4.1 Toolbar

| Region | Content |
|---|---|
| Left | Open-file button (system dialog), db path chip (truncated), recent-files dropdown |
| Right | RW/RO toggle, preferences gear |

RW/RO toggle:
- Read-only: neutral chip, lock icon, `READ-ONLY` text.
- Read-write: `--warning` filled chip with a 2 px halo ring, 600-weight, tracking +0.06em. Plus: a **3 px filled `--warning` strip across the very top of the window** (above the toolbar) so the state is legible from anywhere. Hovering shows "Toggle to return to read-only". Toggling into read-write opens a one-time-per-session confirmation: "Enable write mode? The app will allow data and schema mutations."
- Keyboard shortcut: ⌘⇧W.

### 4.2 SchemaTree (sidebar)

- Groups: **Tables**, **Views**, **Indexes**, **Triggers** (empty groups hidden).
- Each group disclosure: left-pointing caret at 8 px, rotates 90° on expand, `--dur-fast`.
- Row: 28 px tall, monospace name, `--text-muted` row-count suffix (`users  ·  1,204`).
- Selection: full-row fill with `--bg-row-selected`, 2 px `--accent` left bar.
- Keyboard: ↑/↓ to move, → expands, ← collapses / jumps to parent, Enter opens in Browse tab, ⌘Enter opens in new tab (v2).
- **Selecting a table from the sidebar always routes to the Browse tab**, regardless of which tab was active. Picking a table is a "show me the data" intent — don't leave the user looking at an unrelated tab.
- Search: pressing `/` focuses a small filter field at the top of the sidebar. Matches by substring on table name; filters the tree.

### 4.3 Tabs (main area)

- Three fixed tabs: **Browse**, **Query**, **Schema**. Always visible; no close button.
- 32 px tall. Selected tab has `--accent` 2 px bottom underline.
- Switching tabs preserves each tab's state (selected table, open query, etc.).

### 4.4 DataGrid (Browse tab)

Built on TanStack Table v8.

**Header row** (sticky, 28 px):
- Column name in `--text`, decl-type in `--text-muted` beneath (2-line mode, `--text-xs`). Right-click → column menu (hide / pin / sort).
- PK columns: ⚷ glyph prefix. FK columns: ↗ glyph prefix (click = follow foreign key, v2).
- Sort indicator: small ▲/▼, click cycles asc → desc → none.

**Body rows** (24 px):
- Zebra stripes with `--bg-row-alt`.
- Cell padding: 0 4px, monospace, single line with ellipsis on overflow. NULL rendered as italic `--text-muted` `NULL` string. BLOB rendered as `<blob · Nb>` badge.
- Row hover: no background change (too busy). Selection via click: `--bg-row-selected`. Shift/⌘-click extends selection. Multi-row selection drives bulk actions in a row-count chip at the footer.

**Cell edit** (read-write only):
- Double-click enters edit mode. Input replaces cell, same font and padding, 1 px `--accent` outline.
- Enter commits, Esc cancels, ⌘Z undoes last commit.
- Type coercion:
  - `INTEGER` cell: input accepts digits/`-`; bad input shows inline red underline, Enter blocked.
  - `REAL`: digits + `.` + `e`.
  - `TEXT`: any string.
  - `BLOB`: edit disabled (v1); tooltip "Use the Query tab to update BLOBs".
  - `NULL`: context menu "Set NULL" or empty-string heuristic: if cell was NULL and user enters empty, confirm via inline pill "Save as empty string / NULL / Cancel".
- PK columns: locked, show ⚷ on hover. Tooltip: "Primary-key columns can't be edited. Delete and re-insert the row instead."
- Row with pending edit: full-row tint `--bg-row-editing`, left bar in `--warning`.
- Failed commit (constraint error): row tints red, error message appears in the cell's tooltip, row stays dirty until user corrects or Esc cancels.

**Footer** (24 px, always visible):
- Left: row count (`"1,204 rows"`), selected-count when > 0 (`"3 selected"`).
- Center: pagination chip `[◂ 1–100 of 1,204 ▸]` — click opens Go-to-row field.
- Right: `[+ Add row]` (read-write), `[- Delete selected]`, column visibility gear.

**Empty state:**
- No table selected: "Pick a table from the sidebar to browse its rows."
- Table with 0 rows: "No rows yet. [+ Add row]".

**Truncated result warning** (query-driven browse): banner atop grid — "Showing 1,000 of many. [Load more]".

### 4.5 QueryEditor (Query tab)

- Top: Monaco editor, SQL language, `--text-base`, 40% height, resizable splitter.
- Keyboard: ⌘Enter runs, ⌘⇧Enter runs selected text, ⌘S saves to query history (sidebar drawer in v2; local storage in v1).
- Toolbar above editor: Run button (primary, disabled while running), parameters chip (`2 params` when present), history menu.
- Params UI: `?1`, `?2` references detected → inline chip row above the editor lets you type JSON values for each.
- Bottom pane: split between **Results** (DataGrid-compact) and **Error** (monospace red on subtle red bg). One visible at a time.

### 4.6 SchemaView (Schema tab)

Per-table structured panel (sidebar selection drives it).

Sections:
1. **Overview:** name, kind (table/view), row count, creation SQL (collapsed, click to expand).
2. **Columns:** tabular list — `#`, name, type, NOT NULL, DEFAULT, PK. PK-bearing rows highlighted with ⚷ badge.
3. **Foreign keys:** list `from → target_table.to (ON UPDATE x, ON DELETE y)`.
4. **Indexes:** list `name (columns) UNIQUE? origin`.
5. **Triggers** (v2 — show placeholder "No triggers" in v1).

### 4.7 StatusBar

24 px, `--text-xs`, monospace on the right.
- Left: current-mode badge (`READ-ONLY` neutral / `READ-WRITE` warning).
- Middle: last-action status (`Ran query in 12 ms` / `Saved row #42` / `Constraint failed`).
- Right: database meta — `v3.47.0 · UTF-8 · 4096 pg · 120 KB`.

### 4.8 Dialogs and toasts

- Confirm dialog (write-mode enable, delete rows, drop table): centered modal, 400 px wide, ⌘Enter = confirm destructive action, Esc = cancel. Destructive confirm button in `--danger`.
- Toast: top-right, stack of up to 3. Auto-dismiss: info / success at 4 s, errors at 8 s. Click to dismiss early. Slide-in from right, `--dur`. Errors use `role="alert"` for screen-reader priority, non-errors use `role="status"`.

## 5. States matrix

| Component | Default | Hover | Focus | Active/Pressed | Disabled | Busy | Error |
|---|---|---|---|---|---|---|---|
| Button primary | filled accent | accent +5% brightness | 2 px `--focus-ring` | filled accent -5% | 40% opacity, no hover | spinner in place of label | border `--danger` |
| Button ghost | transparent, `--text` | `--bg-panel` tint | ring | tint -5% | 40% opacity | spinner | n/a |
| Button danger | **filled `--danger`** + `--text-inverse` | brightness +5% | ring | -5% | outlined only: `--danger` text + 40% opacity border | n/a | n/a |
| Input | 1 px `--border` | 1 px `--border-strong` | 2 px `--focus-ring` + 1 px accent border | same as focus | 40% opacity | n/a | 1 px `--danger` border |
| Row (grid) | default | (none) | 1 px accent inner border | selected fill | n/a | (cell spinner for cell-level) | cell tinted `--danger` bg |
| Tab | `--text-muted` | `--text` | ring | `--text` + 2 px `--accent` underline | n/a | n/a | n/a |

## 6. Keyboard map

| Shortcut | Action |
|---|---|
| ⌘O | Open database file |
| ⌘W | Close database |
| ⌘⇧W | Toggle read/write mode |
| ⌘1 / ⌘2 / ⌘3 | Switch to Browse / Query / Schema tab |
| ⌘+ / ⌘− / ⌘0 | Zoom in / out / reset (persisted across reloads) |
| / | Focus sidebar filter |
| ⌘F | Focus find-in-grid (v2) |
| ⌘Enter | Run query (Query tab) / Commit cell edit |
| ⌘⇧Enter | Run selected text (Query tab) |
| ⌘Z / ⌘⇧Z | Undo / Redo last row mutation |
| ⌘N | Add row (Browse, read-write) |
| ⌫ | Delete selected rows (Browse, read-write, prompts) |
| Esc | Cancel cell edit / close modal |
| ⌘K | Open command palette (v2) |

## 7. Accessibility

- **Contrast:** all text ≥ 4.5:1 on its background; verify both light and dark palettes.
- **Focus ring:** always visible when navigating by keyboard; `--focus-ring` 2 px outline, never removed.
- **Semantic roles:** tabs use `role="tablist"` + `role="tab"` + `role="tabpanel"`; grid uses `role="grid"`. Row selection via `aria-selected`.
- **Screen readers:** cell values read with column name ("name, Alice"). NULL announced as "null". Edit mode announces "editing, INTEGER field".
- **Reduced motion:** when `prefers-reduced-motion: reduce`, disable all transitions; state changes snap.

## 8. Open questions (tracked — resolve before v1)

1. **FK navigation:** clicking an FK cell → jump to referenced row. Scoping for v1 vs v2 — default **v2**.
2. **Save queries:** local persistence (file-scoped or global). v1: keep last 20 per DB in localStorage; no UI browser yet.
3. **Multi-statement query editor:** support semicolons. v1 runs only the first statement; warn if multiple detected.
4. **Cell context menu:** depth (Copy / Copy as SQL / Set NULL / Duplicate row / ...). v1: Copy, Set NULL, Delete row.

## 9. Out of scope for v1

- Multi-database tabs / window tiling.
- ER diagram.
- Migrations / schema diffs.
- Encrypted databases (SQLCipher).
- Remote / ATTACH databases.
- Themes beyond light+dark.
- Command palette.
