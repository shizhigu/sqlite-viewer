# Contributing to sqlv

Thanks for considering a contribution! `sqlv` is early-stage and we're actively shaping the architecture — good suggestions land quickly.

## Ground rules

- **Open an issue first** for anything larger than a bug fix. A 3-sentence "here's what I want to do" comment saves everyone a round trip.
- **Changes to the UI go through [`docs/DESIGN.md`](./docs/DESIGN.md) first.** Update the spec, get a 👍, then write the component code. This avoids bikeshed PRs.
- **Keep the CLI's JSON shape stable.** The `SKILL.md` agent workflow depends on it. Additive fields are fine; removing or renaming existing ones is a breaking change.
- **Be kind.** See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).

## Dev loop

Prerequisites: **Rust stable**, **Bun ≥ 1.1**, macOS 13+ or Linux (Windows works but isn't the primary target yet).

```sh
# 1. Clone
git clone https://github.com/shizhigu/sqlite-viewer
cd sqlite-viewer

# 2. Run every test
cargo test --workspace                    # Rust: core + CLI
cd apps/desktop && bun install && bun run test

# 3. Iterate on the CLI
cargo run -p sqlv-cli -- tables --db samples/ecommerce.sqlite

# 4. Iterate on the desktop app
cd apps/desktop && bunx tauri dev
```

## Project layout

```
crates/
  core/     # sqlv-core — shared Rust library; do not depend on a frontend here
  cli/      # sqlv-cli  — `sqlv` binary; clap subcommands in commands/*.rs
  mcp/      # sqlv-mcp  — MCP server (stdio, JSON-RPC) — coming in v0.1
apps/
  desktop/  # Tauri + React + CodeMirror; backend in src-tauri/
docs/       # DESIGN.md is authoritative for UI decisions
samples/    # checked-in sample DBs (regenerate via scripts/)
scripts/    # python generators, release helpers
skills/     # SKILL.md for coding agents
```

## Before you open a PR

Run all of:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop && bun run typecheck && bun run test
```

CI runs the same gates on every push.

## Writing tests

**Core / CLI:** exhaustive is welcome. Cover the happy path, the empty case, boundary values (i64 extremes, 0-length inputs), unicode, NULL vs empty string, and the expected error codes. See `crates/core/tests/` for the style.

**Frontend:** pure-function tests (`src/lib/*.test.ts`) are the highest-value. React-component tests are optional — the desktop is best validated by `tauri dev` + manual exercise.

## Style

- **Rust:** rustfmt defaults. Prefer `thiserror` for library errors and `anyhow` at the CLI boundary. Avoid `.unwrap()` outside tests.
- **TypeScript:** TS strict mode is on. Mirror Rust type names 1:1 in `src/lib/tauri.ts` so diffs are easy to read.
- **Comments:** explain *why*, not what. If a comment would just restate the code, delete it.
- **Commit messages:** imperative mood, present tense. One change per commit.

## Proposing a feature

Open an issue using the "Feature request" template. In one paragraph, answer:

1. What user problem does this solve?
2. What's the proposed UX (CLI flag / UI gesture / MCP tool)?
3. What alternatives did you consider?

Maintainers will either label it `accepted` (please go for it) or explain why it's out of scope. See the "On purpose, not happening" list in the [README roadmap](./README.md#roadmap).

## Thanks

Your name is going in the credits if you ship code. 💛
