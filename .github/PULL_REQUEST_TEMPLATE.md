<!-- Thanks for the PR! Fill this in so reviewers can evaluate quickly. -->

## Summary

<!-- 1-3 sentences on what this changes and why. -->

## Linked issue

Fixes #

## What changed

- [ ] CLI
- [ ] Desktop app
- [ ] MCP server
- [ ] sqlv-core (shared library)
- [ ] Docs / SKILL.md / DESIGN.md
- [ ] CI / release tooling

## Test plan

<!--
Describe how you verified this.
- `cargo test --workspace`
- `bun run typecheck && bun run test`
- Manual: what did you click? what did you type?
-->

## Screenshots / screen recordings

<!-- If this touches the UI, attach before/after. -->

## Checklist

- [ ] Updated `docs/DESIGN.md` if UI visuals changed
- [ ] Added tests for new behavior (or explained why not)
- [ ] No new `.unwrap()` in production code paths
- [ ] Ran `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Ran `bun run typecheck` and `bun run test` in `apps/desktop`
