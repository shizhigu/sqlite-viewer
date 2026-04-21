use is_terminal::IsTerminal;
use serde::Serialize;
use std::io::{self, Write};

/// Emit a successful result. We always produce JSON on stdout — the `--table`
/// pretty view is a future enhancement; `--json` remains for explicit opt-in
/// and is implicit whenever stdout is not a TTY (agent / pipe consumption).
pub fn emit<T: Serialize>(value: &T, force_json: bool) {
    let _ = (force_json, io::stdout().is_terminal()); // reserved for later routing
    let mut out = io::stdout().lock();
    let _ = serde_json::to_writer_pretty(&mut out, value);
    let _ = out.write_all(b"\n");
}

/// Emit an error to stderr as `{"error": {"code": "...", "message": "..."}}`.
pub fn emit_error(code: &str, message: &str) {
    let payload = serde_json::json!({
        "error": {"code": code, "message": message}
    });
    let mut err = io::stderr().lock();
    let _ = serde_json::to_writer_pretty(&mut err, &payload);
    let _ = err.write_all(b"\n");
}
