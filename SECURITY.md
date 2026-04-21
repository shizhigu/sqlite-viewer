# Security policy

## Supported versions

Only the `main` branch is currently supported — `sqlv` is pre-1.0 and we ship fixes forward rather than backporting.

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

Instead, use GitHub's private vulnerability reporting:

1. Go to the [Security tab](https://github.com/shizhigu/sqlite-viewer/security).
2. Click "Report a vulnerability".
3. Provide a description, reproduction steps, and impact assessment.

You should get a response within 72 hours. If the report is valid, we'll work with you on a coordinated disclosure timeline (typically 30–90 days depending on severity).

## Threat model

`sqlv` is a **local** tool. Its threat model is narrow:

- The CLI runs with the user's own permissions. No elevation. Its attack surface is the databases it's pointed at.
- The desktop app's push-server binds **127.0.0.1 only** and requires an auth token (written to `~/.sqlv/desktop.token`, mode 0600). It does **not** accept remote connections.
- The MCP server reads from stdin only (no network).
- Tauri's renderer process has strict CSP and no filesystem access except via explicit IPC commands.

### Out of scope

- Attacks requiring physical access to an unlocked machine.
- Attacks requiring the user to manually run a malicious SQL file.
- Flaws in SQLite itself (report those to [sqlite.org](https://sqlite.org/)).

## Scope of typical issues

In scope:

- Any way to run SQL against a database the user didn't open
- Any way to bypass the `--write` / read-only gate from a read-only connection
- Any way for a non-root local process to read the auth token, bypass it, or use it after desktop exits
- Any way to inject SQL via identifier handling (table/column/pragma names)
- Privilege escalation via the Tauri IPC surface

Out of scope:

- SQL injection in queries the user explicitly wrote (that's the user's responsibility)
- Issues that only manifest if you run a binary from an untrusted source
- DoS via repeatedly calling `sqlv push`
