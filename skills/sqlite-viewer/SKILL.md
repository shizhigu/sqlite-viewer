---
name: sqlite-viewer
description: Inspect and (with explicit consent) edit SQLite databases from the shell. Use when the user mentions a `.sqlite` / `.db` / `.sqlite3` file, asks to "look at the database", debug a schema, count rows, check foreign keys, export data, or run an ad-hoc SQL query. Do NOT use for remote databases, non-SQLite formats, or one-off computation that doesn't involve a database file.
---

# sqlite-viewer

A single CLI, `sqlv`, provides structured (JSON-first) access to SQLite databases. It is designed for non-interactive use: every command emits JSON on stdout, errors as JSON on stderr, and uses stable exit codes — you never need to parse tables or prose.

## When to use

Reach for `sqlv` when any of these are true:

- The user pointed at a file ending in `.sqlite`, `.sqlite3`, or `.db`.
- The user asks "what's in this database", "how many rows", "what's the schema", "what tables exist".
- The user wants to run SQL against a local SQLite file (ad-hoc exploration, debugging, data migration staging).
- The user wants to export a SQLite database as SQL.

Do NOT use `sqlv` for:

- Postgres / MySQL / SQL Server / BigQuery — `sqlv` is SQLite-only.
- Mutating schema or data on a database you haven't been explicitly given write permission for — always confirm first, see [Writes](#writes).

## Precondition: check the binary is installed

Before the first call, verify:

```sh
sqlv --version
```

If it is not on `PATH`, tell the user how to install it (from the project's release page, Homebrew once published, or `cargo install --path crates/cli` from source) and stop.

## Discovery workflow

Always start from the outside in. Don't guess table names — ask SQLite.

```sh
# 1. Metadata: page size, encoding, sqlite lib version, read/write mode.
sqlv open --db PATH

# 2. Tables with row counts.
sqlv tables --db PATH

# 3. Full schema of a specific table.
sqlv schema --db PATH <table>

# 4. (Optional) All user tables at once.
sqlv schema --db PATH
```

`sqlv tables` returns an array of `{ name, kind, row_count, sql }`; `sqlv schema <table>` returns `{ columns, foreign_keys, indexes, sql }`. Parse the JSON — do not string-match against the `sql` field for column info.

## Querying

```sh
sqlv query --db PATH "<SQL>" [-p <JSON>]... [--limit N] [--offset N]
```

- Parameters bind to `?1`, `?2`, ... Each `-p` takes a **JSON literal**, so wrap strings:
  `-p '"Alice"'`, `-p 42`, `-p null`, `-p true`.
- Default `--limit` is 1000. The result includes `"truncated": true` if more rows exist — re-run with a higher limit or paginate via `--offset`.
- Returns JSON: `{ columns, column_types, rows, truncated, elapsed_ms }`.
- Rows are arrays of values; `null` is JSON null; blobs are `{"$blob_base64": "..."}`.

Example:

```sh
sqlv query --db ./app.sqlite \
  "SELECT id, email FROM users WHERE created_at >= ?1 ORDER BY id LIMIT ?2" \
  -p '"2026-01-01"' -p 50
```

## Writes

Writes are gated. Default is read-only. To mutate you must pass **`--write`** on any mutating command.

**Before running any write, always:**

1. Show the user the exact SQL you intend to run (copy-pasted, not paraphrased).
2. Explain the blast radius (row counts, which tables).
3. Ask for explicit confirmation. "Shall I run this write?" — wait for yes.
4. Then execute:

```sh
sqlv exec --db PATH --write "<SQL>" [-p <JSON>]...
```

The result is `{ rows_affected, last_insert_rowid, elapsed_ms }`. Note: `rows_affected` after a DDL statement (CREATE/DROP/ALTER) reflects the last prior DML — treat it as meaningful only for INSERT/UPDATE/DELETE.

### PRAGMA writes

Same pattern — `sqlv pragma --db PATH <name> <value> --write`.
Values must be numeric, a bare keyword, or a single-quoted literal. Anything else is rejected for injection safety.

### Backups before destructive writes

If the user is running a DELETE, DROP, TRUNCATE, or broad UPDATE on a file they care about, suggest copying the DB first:

```sh
cp ./app.sqlite ./app.backup.sqlite
```

## Error handling

Every error is JSON on stderr and a specific exit code:

| Exit | `code` | Meaning |
|---|---|---|
| 0 | — | Success |
| 1 | `io` / `invalid` / `other` | Generic failure |
| 2 | `usage` | Missing/conflicting flags (from clap) or a `sqlv`-enforced policy (e.g. writing without `--write`) |
| 3 | `not_found` | Referenced table/view does not exist |
| 4 | `readonly` | Tried to mutate a connection opened read-only |
| 5 | `sql` | SQL parse or constraint error |

Stderr payload shape:

```json
{ "error": { "code": "sql", "message": "no such table: users" } }
```

Branch on `code`, not on the message string (the message may be localized or refined).

## Common recipes

### Count rows in every table

```sh
sqlv tables --db PATH | jq 'map({name, row_count})'
```

### Find tables with zero rows

```sh
sqlv tables --db PATH | jq '[.[] | select(.row_count == 0) | .name]'
```

### Describe every column in the database

```sh
sqlv schema --db PATH | jq '[.[] | {table: .name, columns: [.columns[].name]}]'
```

### Export table as JSON rows

```sh
sqlv query --db PATH "SELECT * FROM orders" --limit 100000 | jq '.rows'
```

### Dump schema for a fresh copy

```sh
sqlv dump --db PATH --schema-only > schema.sql
```

### Full dump (schema + data)

```sh
sqlv dump --db PATH > backup.sql
```

### Investigate a slow query

```sh
sqlv query --db PATH "EXPLAIN QUERY PLAN SELECT ..."
```

## Safety rules

1. **Never run destructive SQL unprompted.** Always show, explain, confirm, then run.
2. **Never pass `--write` opportunistically.** It's a deliberate opt-in gate. If the user hasn't asked to modify data, don't.
3. **Never modify `sqlite_master`, `sqlite_sequence`, or `sqlite_stat*`.** Use the proper DDL or PRAGMA instead.
4. **Respect `truncated`.** If a query result has `"truncated": true`, either tell the user the result is partial or re-run with a higher `--limit`. Do not silently summarize partial data.
5. **Treat large DUMP output with care.** Don't echo a 100 MB dump to the chat — redirect to a file.
6. **Unknown databases are read-only.** If the user drops a file on you without context, use only read commands until they say otherwise.

## MCP server (native tools for Claude Desktop / Cursor / Zed)

If the host supports MCP (Model Context Protocol), prefer native tools over shelling out to the CLI. Install `sqlv-mcp` (shipped in releases) and add it to the host's MCP server list.

### Claude Desktop (`~/Library/Application Support/Claude/claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "sqlv": {
      "command": "sqlv-mcp",
      "args": []
    }
  }
}
```

### Claude Code / Cursor / Windsurf / Zed

Same shape — `command: sqlv-mcp`, no args. Stdio transport. The host will discover these tools:

- `sqlv_open(path, read_only?)` — open a SQLite file. Always pass absolute paths.
- `sqlv_tables()` / `sqlv_views()`
- `sqlv_schema(name)`
- `sqlv_query(sql, params?, limit?, offset?)` — read-only SELECT.
- `sqlv_exec(sql, params?, confirm_destructive: true)` — writes. The `confirm_destructive` flag is **required** — show the SQL to the user first and only pass `true` after they agree.
- `sqlv_stats()`

### MCP error codes

Errors come back as JSON-RPC errors with these codes:

| Code | Meaning |
|---|---|
| -32001 | `not_found` (table/view doesn't exist) |
| -32002 | `readonly` (tried to write on RO connection) |
| -32003 | `sql` (parse / constraint error) |
| -32602 | `invalid` (bad arguments or missing confirm flag) |
| -32000 | generic / no database open |

## Mirroring into the desktop app (`push`, `push-open`)

When the user is running the desktop app (`bunx tauri dev` or the installed build), `sqlv` can send queries and file-open commands **into the live UI**. This is the collaborative loop: you run the command, the human sees the same thing on screen instantly.

- The desktop app listens on `127.0.0.1:50500` (falls back through `50501..=50509` if that port is taken). Localhost only — no network exposure.
- `sqlv push` / `push-open` auto-discover the port.

**Send a query to the desktop app's open DB:**

```sh
sqlv push "SELECT id, name FROM users ORDER BY id LIMIT 20"
```

The desktop app switches to its Query tab, shows your SQL in the editor, runs it, and displays the results. You also receive the same JSON result on stdout, so your workflow is unchanged.

**Ask the desktop to open a specific file:**

```sh
sqlv push-open path/to/app.sqlite           # read-only (safe default)
sqlv push-open path/to/app.sqlite --write   # read-write (requires user consent)
```

Use `push` and `push-open` instead of the plain `query` / `open` commands when the goal is to **show your work to the user as you explore**. Keep using the plain commands when you just need data for your own reasoning.

**Errors from the desktop:**

- Exit code 1 + `"io"`: desktop app isn't running or isn't listening (maybe not started yet).
- Exit code 1 + `"not_open"`: desktop is running but has no DB open — send `push-open` first, or ask the user to open a file.
- Exit code 5 + `"sql"`: SQL parse / constraint error — same semantics as `sqlv query`.

## Command reference (at a glance)

| Command | Reads | Writes (needs `--write`) | Output |
|---|---|---|---|
| `open --db X` | ✓ | | JSON meta |
| `tables --db X` | ✓ | | JSON array |
| `views --db X` | ✓ | | JSON array |
| `indexes --db X [--table T]` | ✓ | | JSON array |
| `schema --db X [<name>]` | ✓ | | JSON (array or object) |
| `query --db X "<SQL>" [-p ...]` | ✓ | | JSON result set |
| `exec --db X --write "<SQL>"` | | ✓ | JSON result |
| `stats --db X` | ✓ | | JSON stats |
| `pragma --db X <name> [value] [--write]` | ✓ | ✓ (with value) | JSON |
| `dump --db X [--schema-only\|--data-only] [--table T]...` | ✓ | | raw SQL |
| `push "<SQL>" [-p ...]` | ✓ (via desktop) | | JSON result, mirrored in UI |
| `push-open <path> [--write]` | ✓ (via desktop) | opt-in | JSON meta, UI opens file |
| `import --db X --table T <file.csv> --write` | | ✓ | JSON `{rows_inserted, elapsed_ms}` |
| `query --db X "..." --stream` | ✓ | | NDJSON: `{type:header}`, `{type:row}`..., `{type:summary}` |

Every command takes a global `--json` (forces JSON output, default when stdout isn't a TTY) and `--quiet`.
