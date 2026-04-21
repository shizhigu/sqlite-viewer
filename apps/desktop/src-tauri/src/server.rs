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
use sqlv_core::{Db, OpenOpts, Page, Value};
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
}

#[derive(Serialize, Clone)]
pub struct PushedEvent {
    pub sql: String,
    pub result: Option<sqlv_core::QueryResult>,
    pub error: Option<AppError>,
    pub token: u64,
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
        return req.respond(json_error(401, "unauthorized", "missing or invalid X-Sqlv-Token header"));
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
            AppError { code: "invalid".into(), message: format!("invalid JSON: {e}") },
        )
    })?;

    let guard = lock_state(state);
    let db = match guard.as_ref() {
        Some(db) => db,
        None => {
            let ev = PushedEvent {
                sql: req.sql.clone(),
                result: None,
                error: Some(AppError::not_open()),
                token,
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

fn handle_open(
    app: &AppHandle,
    state: &Arc<AppState>,
    body: &str,
    token: u64,
) -> Result<Response<std::io::Cursor<Vec<u8>>>, (u16, AppError)> {
    let req: OpenRequest = serde_json::from_str(body).map_err(|e| {
        (
            400,
            AppError { code: "invalid".into(), message: format!("invalid JSON: {e}") },
        )
    })?;

    let read_only = req.read_only.unwrap_or(true);
    let path = PathBuf::from(&req.path);
    match Db::open(&path, OpenOpts { read_only, timeout_ms: Some(5_000) }) {
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
    Response::from_data(body).with_status_code(200).with_header(json_header())
}

fn json_error(status: u16, code: &str, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": { "code": code, "message": message }
    }))
    .unwrap_or_else(|_| br#"{"error":{"code":"other","message":"serialization failed"}}"#.to_vec());
    Response::from_data(body).with_status_code(status).with_header(json_header())
}

fn json_header() -> Header {
    // Both strings are 'static valid HTTP headers — this parse can't fail.
    // The or_else branch is defensive only.
    Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
        .unwrap_or_else(|_| Header::from_bytes(&b"X"[..], &b"x"[..]).unwrap_or_else(|_| unreachable!()))
}

// Existing Instance ref kept alive by the server thread — imported for
// type inference elsewhere if needed.
#[allow(dead_code)]
fn _force_type_usage(_i: &Instance) {}
