<div align="center">

# sqlv

**A SQLite viewer for local database files: a JSON-first CLI, a native desktop app, and an MCP server, all on one shared Rust core.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](./Cargo.toml)
[![CI](https://github.com/shizhigu/sqlite-viewer/actions/workflows/ci.yml/badge.svg)](https://github.com/shizhigu/sqlite-viewer/actions/workflows/ci.yml)

[What it is](#what-it-is) · [Install](#install) · [Quick start](#quick-start) · [Desktop app](#desktop-app) · [Coding agents](#working-with-coding-agents) · [Architecture](#architecture)

</div>

---

## What it is

`sqlv` opens local SQLite files three ways that share the same code:

- A command-line tool that prints JSON, uses stable exit codes, and defaults to read-only.
- A native desktop app (Tauri + React) to browse tables, edit cells, and run SQL.
- An MCP stdio server so an LLM host can call the same operations as structured tools.

It is SQLite only. No Postgres, no MySQL, no remote connections. Every surface (CLI, desktop, MCP) runs through the `sqlv-core` Rust library, so a query behaves the same no matter where you run it.

Databases open read-only by default. Mutations require an explicit opt-in: `--write` on the CLI, a `confirm_destructive` flag over MCP, or flipping the write toggle in the desktop app.

This is an early release (v0.1.0). The core and CLI have thorough test suites (`cargo test` covers query, schema, import, diff, pragma, and the CLI's exit-code and JSON contracts). The MCP server and desktop app are usable but have thin test coverage so far.

## Install

From source (any platform with a Rust toolchain):

```sh
git clone https://github.com/shizhigu/sqlite-viewer
cd sqlite-viewer

# CLI
cargo install --path crates/cli
sqlv --version

# Desktop app (dev)
cd apps/desktop && bun install && bunx tauri dev
```

The release workflow ([`release.yml`](./.github/workflows/release.yml)) builds prebuilt CLI binaries for macOS, Linux, and Windows on each tagged version and uploads them to the [Releases page](https://github.com/shizhigu/sqlite-viewer/releases). A Homebrew formula lives in `packaging/homebrew/` but is not published to a tap yet.

## Quick start

The repo ships a sample e-commerce database (9 tables, 3 views, ~52k rows) so you can try commands immediately:

```sh
# Metadata: page size, encoding, SQLite version, read/write mode
sqlv open --db samples/ecommerce.sqlite

# Tables with row counts
sqlv tables --db samples/ecommerce.sqlite

# Describe one table (columns, foreign keys, indexes)
sqlv schema --db samples/ecommerce.sqlite orders

# Parameterized query. Each -p is a JSON literal.
sqlv query --db samples/ecommerce.sqlite \
  "SELECT id, email FROM customers WHERE id >= ?1 LIMIT 5" -p 100

# Schema-only dump
sqlv dump --db samples/ecommerce.sqlite --schema-only > schema.sql
```

Query output is JSON: `{ columns, column_types, rows, truncated, elapsed_ms }`. Errors go to stderr as `{ "error": { "code": "...", "message": "..." } }` with exit codes `0` ok, `2` usage, `3` not-found, `4` readonly, `5` sql. Branch on the code, not the message.

Other commands: `views`, `indexes`, `exec` (writes), `stats`, `pragma`, `import` (CSV / JSON / JSONL), `maintenance` (vacuum / reindex / analyze / integrity-check / wal-checkpoint), `checkpoint` (snapshot via `VACUUM INTO`), `diff` (schema diff between two files), and `history`. Add `--stream` to `query` for NDJSON output on large result sets.

Regenerate the sample database deterministically with `python3 scripts/make_sample.py`.

## Desktop app

The desktop app is a Tauri shell around a React frontend (CodeMirror editor, TanStack Table with row virtualization, Zustand state). It has three tabs: Browse, Query, and Schema, plus a schema tree in the sidebar.

The write mode is deliberately hard to forget: read-only until you flip the toggle, and read-write mode paints a warning strip across the top of the window. Grid edits can be staged into a pending-changes panel and committed in a single transaction. A running query can be cancelled from the toolbar (the backend calls SQLite's interrupt handle).

The UI is specified in [`docs/DESIGN.md`](./docs/DESIGN.md): information-dense and keyboard-first, native-feeling on macOS, calm neutral palette with red/orange/green reserved for error, warning, and success states rather than decoration. The doc is the source of truth for visual decisions.

### The push loop

You can mirror what you run in a terminal into the running desktop window. With the app open:

```sh
sqlv push-open samples/ecommerce.sqlite
sqlv push "SELECT id, email FROM customers ORDER BY id LIMIT 20"
```

The desktop switches to the Query tab, shows the SQL in the editor, runs it, and renders the grid, so a person watching the window sees exactly what was run. You still get the JSON result on stdout. Read-only queries run automatically; anything that looks like a write is staged in the editor for a human to approve unless you pass `--run`.

This works over a loopback HTTP server (127.0.0.1 only, ports 50500 to 50509) that requires a per-session auth token. It never listens on a public interface.

## Working with coding agents

`sqlv` is a tool built to be driven by coding agents, not an agent itself. It contains no LLM and makes no model calls. What it provides is the agent-facing surface area:

- A [SKILL.md](./skills/sqlite-viewer/SKILL.md) that teaches an agent when to reach for `sqlv`, the `tables → schema → query` discovery flow, how to branch on error codes, and the show-explain-confirm rule before any write. Drop it into a Claude Code skills directory and the agent invokes `sqlv` when a `.sqlite` / `.db` file comes up.
- An MCP stdio server (`sqlv-mcp`) that speaks JSON-RPC and advertises `sqlv_open`, `sqlv_tables`, `sqlv_views`, `sqlv_schema`, `sqlv_query`, `sqlv_exec`, and `sqlv_stats`, plus `sqlv_push_open` and `sqlv_push_query` for the push loop below. Writes require an explicit `confirm_destructive: true`. It holds the open database between calls so an agent opens once and queries many times. The code targets Claude Desktop, Claude Code, Cursor, and Zed hosts (config snippets are in [SKILL.md](./skills/sqlite-viewer/SKILL.md)).
- The `push` loop above, so an agent's exploration is visible to the human in real time instead of pasted into a chat.

Every query, exec, open, and cancel from the UI, CLI, or MCP is recorded in `~/.sqlv/activity.db` with a source tag (`ui` / `cli` / `agent`). `sqlv history` searches that shared log.

## Architecture

```
        CLI (sqlv)          Desktop (Tauri + React)        MCP server (sqlv-mcp)
   clap, JSON stdout        CodeMirror + TanStack           stdio JSON-RPC
            \                        |                              /
             \                       |                             /
              +------------- sqlv-core (Rust library) ------------+
                   open (read-only / read-write), typed schema
                   introspection, parameterized query + exec,
                   stats, injection-safe pragma, dump, diff,
                   import, activity log, JSON-stable Value enum
                                     |
                        rusqlite (bundled SQLite)
```

Four crates in a Cargo workspace: `crates/core`, `crates/cli`, `crates/mcp`, and `apps/desktop/src-tauri`. Because all three frontends call into `sqlv-core`, their semantics cannot drift apart.

CI runs `fmt`, `clippy`, and the core + CLI test suite on Linux and macOS, plus a frontend typecheck, unit tests, and Vite build. See [`.github/workflows/ci.yml`](./.github/workflows/ci.yml).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). Good places to start reading: [`docs/DESIGN.md`](./docs/DESIGN.md) for UI work, [`skills/sqlite-viewer/SKILL.md`](./skills/sqlite-viewer/SKILL.md) for agent behavior, and [`crates/core/`](./crates/core) for the shared library. Run the tests with `cargo test --workspace` and, for the frontend, `cd apps/desktop && bun run test`.

## License

[MIT](./LICENSE). See [SECURITY.md](./SECURITY.md) for vulnerability reporting.
