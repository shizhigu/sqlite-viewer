//! Local HTTP loopback server for external tools (the `sqlv push` CLI,
//! or any script/agent) to stream SQL into the running desktop app.
//!
//! - Binds 127.0.0.1 only — never accepts remote connections.
//! - Every request except `GET /health` requires the `X-Sqlv-Token` header
//!   matching the token written to `~/.sqlv/instances/<pid>.json`.
//! - Port falls back through 50500..=50509 so multiple instances coexist.
//! - Each running instance publishes its `{pid, port, token}` so the CLI
//!   can locate it without port-scanning.
//!
//! Routes:
//!   POST /query    { "sql": "...", "params": [...]? }    →  JSON QueryResult
//!   POST /open     { "path": "...", "read_only": bool? } →  JSON DbMeta
//!   GET  /health                                         →  { "ok": true }
//!
//! Every successful push also emits a Tauri event (`pushed-query` /
//! `pushed-open`) so the UI mirrors the action in real time.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};
use sqlv_core::{classify_sql, Db, OpenOpts, Page, SqlKind, Value};
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::discovery::{self, Instance};
use crate::error::AppError;
use crate::state::AppState;

const DEFAULT_PORT: u16 = 50_500;
const PORT_RANGE: u16 = 10; // 50500..=50509
const AUTH_HEADER: &str = "X-Sqlv-Token";

#[derive(Deserialize)]
struct PushRequest {
    sql: String,
    #[serde(default)]
    params: Vec<serde_json::Value>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
    /// Execution policy:
    ///   - `"auto"` (default): SELECT/EXPLAIN run immediately; anything
    ///     that looks like a write populates the editor as pending and
    ///     waits for the human to click Run.
    ///   - `"run"`: always execute regardless of classification. Use from
    ///     trusted agent loops that have already consented.
    ///   - `"pending"`: always populate pending — useful for demos.
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PushedEvent {
    pub sql: String,
    pub result: Option<sqlv_core::QueryResult>,
    pub error: Option<AppError>,
    pub token: u64,
    /// `true` when the server did not execute the query — the UI should
    /// populate the editor with the SQL and wait for the user to Run it.
    #[serde(default)]
    pub pending: bool,
    /// Serialized `SqlKind` so the UI can reason about intent.
    pub kind: &'static str,
    /// One row per node of EXPLAIN QUERY PLAN. Populated only when the
    /// push is pending — gives the human something to judge before Run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan: Vec<PlanNode>,
    /// For UPDATE/DELETE pending pushes where we can parse the target,
    /// this reports how many rows the WHERE would affect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affects: Option<AffectedRows>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PlanNode {
    pub id: i64,
    pub parent: i64,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AffectedRows {
    pub table: String,
    pub count: i64,
}

#[derive(Deserialize)]
struct OpenRequest {
    path: String,
    #[serde(default)]
    read_only: Option<bool>,
}

#[derive(Serialize, Clone)]
pub struct PushedOpenEvent {
    pub path: String,
    pub read_only: bool,
    pub meta: Option<sqlv_core::DbMeta>,
    pub error: Option<AppError>,
    pub token: u64,
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    thread::spawn(move || {
        let Some((server, port)) = bind() else {
            eprintln!(
                "[sqlv] could not bind local HTTP port in range {}..={}; push disabled",
                DEFAULT_PORT,
                DEFAULT_PORT + PORT_RANGE - 1
            );
            return;
        };

        // Register ourselves in ~/.sqlv/instances/<pid>.json and keep the
        // Instance guard alive for the server's lifetime — it removes the
        // file on drop.
        let instance = match discovery::register(port) {
            Ok(inst) => inst,
            Err(e) => {
                eprintln!("[sqlv] failed to write instance file: {e}; push disabled");
                return;
            }
        };
        let auth_token = instance.info.token.clone();
        eprintln!("[sqlv] push server listening on http://127.0.0.1:{port}");

        let mut seq: u64 = 1;
        for req in server.incoming_requests() {
            let response = route(&app, &state, &auth_token, req, &mut seq);
            // We always emit a response before moving on; route() returns
            // Ok(()) whether it succeeded or wrote an error. Any late panic
            // inside the closure is caught by the thread's boundary but
            // we take care not to panic in the first place.
            if let Err(e) = response {
                eprintln!("[sqlv] response write failed: {e}");
            }
        }
        // Keep `instance` alive until the loop exits — Drop cleans the file.
        drop(instance);
    });
}

fn route(
    app: &AppHandle,
    state: &Arc<AppState>,
    auth_token: &str,
    mut req: Request,
    seq: &mut u64,
) -> std::io::Result<()> {
    let url = req.url().to_string();
    let method = req.method().clone();

    // Health has no auth — used by CLIs to probe liveness.
    if matches!(method, Method::Get) && url == "/health" {
        return req.respond(json_ok(&serde_json::json!({
            "ok": true,
            "service": "sqlv-desktop",
            "version": env!("CARGO_PKG_VERSION"),
        })));
    }

    if !is_authorized(&req, auth_token) {
        return req.respond(json_error(
            401,
            "unauthorized",
            "missing or invalid X-Sqlv-Token header",
        ));
    }

    match (method, url.as_str()) {
        (Method::Post, "/query") => {
            let body = read_body(&mut req);
            let resp = match body.and_then(|b| handle_query(app, state, &b, *seq)) {
                Ok(r) => {
                    *seq += 1;
                    r
                }
                Err((status, err)) => json_error(status, &err.code, &err.message),
            };
            req.respond(resp)
        }
        (Method::Post, "/open") => {
            let body = read_body(&mut req);
            let resp = match body.and_then(|b| handle_open(app, state, &b, *seq)) {
                Ok(r) => {
                    *seq += 1;
                    r
                }
                Err((status, err)) => json_error(status, &err.code, &err.message),
            };
            req.respond(resp)
        }
        _ => req.respond(json_error(404, "not_found", "unknown route")),
    }
}

fn is_authorized(req: &Request, expected: &str) -> bool {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(AUTH_HEADER))
        .map(|h| constant_time_eq(h.value.as_str().as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
}

/// Constant-time byte-string compare. Overkill for a local token but cheap.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn read_body(req: &mut Request) -> Result<String, (u16, AppError)> {
    let mut s = String::new();
    req.as_reader().read_to_string(&mut s).map_err(|e| {
        (
            400,
            AppError {
                code: "invalid".into(),
                message: format!("unreadable body: {e}"),
            },
        )
    })?;
    Ok(s)
}

fn bind() -> Option<(Server, u16)> {
    for offset in 0..PORT_RANGE {
        let port = DEFAULT_PORT + offset;
        if let Ok(s) = Server::http(("127.0.0.1", port)) {
            return Some((s, port));
        }
    }
    None
}

fn handle_query(
    app: &AppHandle,
    state: &Arc<AppState>,
    body: &str,
    token: u64,
) -> Result<Response<std::io::Cursor<Vec<u8>>>, (u16, AppError)> {
    let req: PushRequest = serde_json::from_str(body).map_err(|e| {
        (
            400,
            AppError {
                code: "invalid".into(),
                message: format!("invalid JSON: {e}"),
            },
        )
    })?;

    let mode = req.mode.as_deref().unwrap_or("auto");
    let kind = classify_sql(&req.sql);
    let kind_str: &'static str = match kind {
        SqlKind::ReadOnly => "read_only",
        SqlKind::Mutating => "mutating",
    };

    // Policy for whether to execute:
    //   mode=run     → always execute
    //   mode=pending → never execute (demo / dry-run)
    //   mode=auto    → execute read-only, preview mutating
    let execute = match mode {
        "run" => true,
        "pending" => false,
        _ => matches!(kind, SqlKind::ReadOnly),
    };

    if !execute {
        // Dry-run: populate the editor and let the human approve. While
        // we're here, try to get them some context to judge with: a plan
        // tree from EXPLAIN QUERY PLAN, plus for UPDATE/DELETE a count of
        // the rows the WHERE would actually touch.
        let params: Vec<Value> = req.params.iter().map(Value::from_json).collect();
        let (plan, affects) = {
            let g = lock_state(state);
            if let Some(db) = g.as_ref() {
                let plan = explain_plan(db, &req.sql, &params).unwrap_or_default();
                let affects = estimate_affected_rows(db, &req.sql, &params);
                (plan, affects)
            } else {
                (Vec::new(), None)
            }
        };

        let ev = PushedEvent {
            sql: req.sql.clone(),
            result: None,
            error: None,
            token,
            pending: true,
            kind: kind_str,
            plan,
            affects,
        };
        let _ = app.emit("pushed-query", ev);
        let body = serde_json::json!({
            "pending": true,
            "sql": req.sql,
            "kind": kind_str,
            "message": "Proposed SQL is waiting for user approval in the desktop app.",
        });
        return Ok(Response::from_data(
            serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec()),
        )
        .with_status_code(202)
        .with_header(json_header()));
    }

    let guard = lock_state(state);
    let db = match guard.as_ref() {
        Some(db) => db,
        None => {
            let ev = PushedEvent {
                sql: req.sql.clone(),
                result: None,
                error: Some(AppError::not_open()),
                token,
                pending: false,
                kind: kind_str,
                plan: Vec::new(),
                affects: None,
            };
            let _ = app.emit("pushed-query", ev);
            return Err((409, AppError::not_open()));
        }
    };

    let params: Vec<Value> = req.params.iter().map(Value::from_json).collect();
    let page = Page {
        limit: req.limit.unwrap_or(1_000),
        offset: req.offset.unwrap_or(0),
    };

    match db.query(&req.sql, &params, page) {
        Ok(result) => {
            let ev = PushedEvent {
                sql: req.sql.clone(),
                result: Some(result.clone()),
                error: None,
                token,
                pending: false,
                kind: kind_str,
                plan: Vec::new(),
                affects: None,
            };
            let _ = app.emit("pushed-query", ev);
            Ok(json_ok(&result))
        }
        Err(e) => {
            let err: AppError = e.into();
            let ev = PushedEvent {
                sql: req.sql.clone(),
                result: None,
                error: Some(err.clone()),
                token,
                pending: false,
                kind: kind_str,
                plan: Vec::new(),
                affects: None,
            };
            let _ = app.emit("pushed-query", ev);
            let status = match err.code.as_str() {
                "readonly" => 409,
                "not_found" => 404,
                _ => 400,
            };
            Err((status, err))
        }
    }
}

/// Flatten `EXPLAIN QUERY PLAN <sql>` to a vector of nodes. Errors get
/// swallowed — we're opportunistic here; the preview banner is useful
/// even when EXPLAIN fails.
fn explain_plan(db: &Db, sql: &str, params: &[Value]) -> Option<Vec<PlanNode>> {
    let full = format!("EXPLAIN QUERY PLAN {sql}");
    // Plans are small; cap defensively so a runaway EXPLAIN can't blow
    // memory while the user stares at the preview banner.
    let page = Page {
        limit: 500,
        offset: 0,
    };
    let r = db.query(&full, params, page).ok()?;
    let mut nodes = Vec::new();
    for row in r.rows {
        // PRAGMA shape: (id, parent, notused, detail). Defensive unwraps.
        if row.len() < 4 {
            continue;
        }
        let id = match row[0] {
            Value::Integer(i) => i,
            _ => 0,
        };
        let parent = match row[1] {
            Value::Integer(i) => i,
            _ => 0,
        };
        let detail = match &row[3] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        };
        nodes.push(PlanNode { id, parent, detail });
    }
    Some(nodes)
}

/// For simple UPDATE / DELETE shapes, return the count of rows the WHERE
/// clause matches. Handles the 80% case via regex — complex queries
/// (subqueries, aliases, multi-table DELETE) just don't get an estimate,
/// which is the correct failure mode.
fn estimate_affected_rows(db: &Db, sql: &str, params: &[Value]) -> Option<AffectedRows> {
    // Trim trailing semicolons / whitespace for matching.
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let upper = trimmed.to_ascii_uppercase();

    // UPDATE <table> SET … WHERE …
    let (table, where_clause) = if upper.starts_with("UPDATE") {
        let rest = &trimmed[6..];
        let (table, after) = take_ident(rest)?;
        let after_upper = after.to_ascii_uppercase();
        let set_idx = after_upper.find("SET")?;
        let where_idx = after_upper.find("WHERE")?;
        if where_idx <= set_idx {
            return None;
        }
        (table, &after[where_idx + 5..])
    } else if upper.starts_with("DELETE FROM") {
        let rest = &trimmed[11..];
        let (table, after) = take_ident(rest.trim_start())?;
        let after_upper = after.to_ascii_uppercase();
        let where_idx = after_upper.find("WHERE")?;
        (table, &after[where_idx + 5..])
    } else {
        return None;
    };

    let count_sql = format!(
        "SELECT COUNT(*) FROM {} WHERE {}",
        quote_ident_best_effort(&table),
        where_clause.trim()
    );
    let page = Page::default();
    let res = db.query(&count_sql, params, page).ok()?;
    let count = match res.rows.first().and_then(|r| r.first()) {
        Some(Value::Integer(n)) => *n,
        _ => return None,
    };
    Some(AffectedRows { table, count })
}

fn take_ident(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let mut chars = s.char_indices();
    let mut end = 0;
    let mut name = String::new();
    if let Some((_, c)) = chars.next() {
        if c == '"' {
            // Quoted identifier.
            for (i, cc) in chars {
                end = i + cc.len_utf8();
                if cc == '"' {
                    return Some((name, &s[end..]));
                }
                name.push(cc);
            }
            return None;
        } else if c.is_ascii_alphabetic() || c == '_' {
            name.push(c);
            end = c.len_utf8();
        } else {
            return None;
        }
    }
    for (i, c) in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    Some((name, &s[end..]))
}

fn quote_ident_best_effort(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn handle_open(
    app: &AppHandle,
    state: &Arc<AppState>,
    body: &str,
    token: u64,
) -> Result<Response<std::io::Cursor<Vec<u8>>>, (u16, AppError)> {
    let req: OpenRequest = serde_json::from_str(body).map_err(|e| {
        (
            400,
            AppError {
                code: "invalid".into(),
                message: format!("invalid JSON: {e}"),
            },
        )
    })?;

    let read_only = req.read_only.unwrap_or(true);
    let path = PathBuf::from(&req.path);
    match Db::open(
        &path,
        OpenOpts {
            read_only,
            timeout_ms: Some(5_000),
        },
    ) {
        Ok(db) => match db.meta() {
            Ok(meta) => {
                *lock_state(state) = Some(db);
                let ev = PushedOpenEvent {
                    path: req.path.clone(),
                    read_only,
                    meta: Some(meta.clone()),
                    error: None,
                    token,
                };
                let _ = app.emit("pushed-open", ev);
                Ok(json_ok(&meta))
            }
            Err(e) => {
                let err: AppError = e.into();
                let ev = PushedOpenEvent {
                    path: req.path,
                    read_only,
                    meta: None,
                    error: Some(err.clone()),
                    token,
                };
                let _ = app.emit("pushed-open", ev);
                Err((400, err))
            }
        },
        Err(e) => {
            let err: AppError = e.into();
            let ev = PushedOpenEvent {
                path: req.path,
                read_only,
                meta: None,
                error: Some(err.clone()),
                token,
            };
            let _ = app.emit("pushed-open", ev);
            Err((400, err))
        }
    }
}

/// Lock the shared state, recovering from poison (another thread panicked
/// with the guard held). Poison recovery is safe here because `AppState`
/// only wraps an `Option<Db>` — there's no invariant to corrupt.
fn lock_state(state: &Arc<AppState>) -> std::sync::MutexGuard<'_, Option<Db>> {
    match state.current.lock() {
        Ok(g) => g,
        Err(p) => {
            eprintln!("[sqlv] state mutex poisoned, recovering");
            p.into_inner()
        }
    }
}

fn json_ok<T: Serialize>(value: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec());
    Response::from_data(body)
        .with_status_code(200)
        .with_header(json_header())
}

fn json_error(status: u16, code: &str, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": { "code": code, "message": message }
    }))
    .unwrap_or_else(|_| br#"{"error":{"code":"other","message":"serialization failed"}}"#.to_vec());
    Response::from_data(body)
        .with_status_code(status)
        .with_header(json_header())
}

fn json_header() -> Header {
    // Both strings are 'static valid HTTP headers — this parse can't fail.
    // The or_else branch is defensive only.
    Header::from_bytes(
        &b"Content-Type"[..],
        &b"application/json; charset=utf-8"[..],
    )
    .unwrap_or_else(|_| Header::from_bytes(&b"X"[..], &b"x"[..]).unwrap_or_else(|_| unreachable!()))
}

// Existing Instance ref kept alive by the server thread — imported for
// type inference elsewhere if needed.
#[allow(dead_code)]
fn _force_type_usage(_i: &Instance) {}
