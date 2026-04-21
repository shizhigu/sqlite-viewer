//! sqlv-mcp — Minimal MCP (Model Context Protocol) stdio server.
//!
//! Speaks newline-delimited JSON-RPC 2.0 on stdin/stdout.
//!
//! Implements the subset of MCP needed by Claude Desktop / Claude Code /
//! Cursor / Zed hosts:
//!   - `initialize`                 — handshake
//!   - `notifications/initialized`  — drop on the floor
//!   - `tools/list`                 — advertises the sqlv tools
//!   - `tools/call`                 — dispatches to sqlv-core
//!
//! Held state: the currently-open `Db`, so that an agent can `sqlv_open` once
//! and then issue many `sqlv_query` / `sqlv_schema` calls without re-opening
//! the file each time.
//!
//! Every tool returns JSON with stable field names — same shapes as the CLI
//! so agents can reuse the same parsing logic across both transports.

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};
use sqlv_core::{Db, OpenOpts, Page, Value as SqlValue};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "sqlv";

struct Server {
    db: Mutex<Option<Db>>,
}

impl Server {
    fn new() -> Self {
        Self {
            db: Mutex::new(None),
        }
    }
}

fn main() {
    let server = Server::new();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[sqlv-mcp] bad JSON-RPC line: {e}");
                continue;
            }
        };
        let response = handle(&server, &req);
        if let Some(r) = response {
            if serde_json::to_writer(&mut writer, &r).is_err() {
                break;
            }
            if writer.write_all(b"\n").is_err() {
                break;
            }
            if writer.flush().is_err() {
                break;
            }
        }
    }
}

fn handle(server: &Server, req: &Value) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str())?;
    let id = req.get("id").cloned();
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

    // Notifications (no id) never get a response.
    let is_notification = id.is_none();

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            },
        })),
        "notifications/initialized" => return None,
        "tools/list" => Ok(tools_list()),
        "tools/call" => call_tool(server, &params),
        "ping" => Ok(json!({})),
        other => Err(JsonRpcError {
            code: -32601,
            message: format!("method not found: {other}"),
        }),
    };

    if is_notification {
        return None;
    }

    Some(match result {
        Ok(r) => json!({"jsonrpc":"2.0","id":id,"result":r}),
        Err(e) => json!({"jsonrpc":"2.0","id":id,"error":{"code":e.code,"message":e.message}}),
    })
}

struct JsonRpcError {
    code: i32,
    message: String,
}

fn tools_list() -> Value {
    json!({
        "tools": [
            tool_def(
                "sqlv_open",
                "Open a SQLite database file. Read-only by default.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the .sqlite/.db file." },
                        "read_only": { "type": "boolean", "default": true, "description": "Open RO (default) or RW." }
                    },
                    "required": ["path"]
                }),
            ),
            tool_def(
                "sqlv_tables",
                "List user tables (excluding sqlite_* internals) with row counts.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool_def(
                "sqlv_views",
                "List views.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool_def(
                "sqlv_schema",
                "Describe the schema of one table or view. Returns columns, PK, FK, indexes.",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Table or view name." }
                    },
                    "required": ["name"]
                }),
            ),
            tool_def(
                "sqlv_query",
                "Execute a read-only SQL SELECT and return up to `limit` rows as JSON.",
                json!({
                    "type": "object",
                    "properties": {
                        "sql": { "type": "string" },
                        "params": { "type": "array", "items": {}, "description": "Positional JSON values bound to ?1, ?2, ..." },
                        "limit": { "type": "integer", "default": 1000 },
                        "offset": { "type": "integer", "default": 0 }
                    },
                    "required": ["sql"]
                }),
            ),
            tool_def(
                "sqlv_exec",
                "Execute a mutating SQL statement (INSERT/UPDATE/DELETE/DDL). Requires `confirm_destructive: true`, the database must have been opened with read_only:false, and the caller should show the SQL to the user first.",
                json!({
                    "type": "object",
                    "properties": {
                        "sql": { "type": "string" },
                        "params": { "type": "array", "items": {} },
                        "confirm_destructive": { "type": "boolean", "description": "Must be true — forces the agent to acknowledge the mutation." }
                    },
                    "required": ["sql", "confirm_destructive"]
                }),
            ),
            tool_def(
                "sqlv_stats",
                "Per-table row counts + database-level stats (size, page count, freelist).",
                json!({ "type": "object", "properties": {} }),
            ),
            tool_def(
                "sqlv_push_query",
                "Forward a SQL query to the running desktop app — mirrors in its Query tab live. Default `mode: auto` executes SELECT/EXPLAIN and stages mutating statements for human approval. Use `mode: run` to bypass the preview when already consented.",
                json!({
                    "type": "object",
                    "properties": {
                        "sql": { "type": "string" },
                        "params": { "type": "array", "items": {} },
                        "limit": { "type": "integer", "default": 1000 },
                        "offset": { "type": "integer", "default": 0 },
                        "mode": { "type": "string", "enum": ["auto", "run", "pending"], "default": "auto" }
                    },
                    "required": ["sql"]
                }),
            ),
            tool_def(
                "sqlv_push_open",
                "Ask the running desktop app to open a database file. Read-only by default.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the .sqlite/.db file." },
                        "read_only": { "type": "boolean", "default": true }
                    },
                    "required": ["path"]
                }),
            ),
        ]
    })
}

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn call_tool(server: &Server, params: &Value) -> Result<Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("tools/call missing `name`"))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let result_json: Value = match name {
        "sqlv_open" => tool_open(server, &args)?,
        "sqlv_tables" => with_db(server, |db| {
            db.tables()
                .map(|t| serde_json::to_value(t).unwrap_or(json!([])))
        })?,
        "sqlv_views" => with_db(server, |db| {
            db.views()
                .map(|t| serde_json::to_value(t).unwrap_or(json!([])))
        })?,
        "sqlv_schema" => tool_schema(server, &args)?,
        "sqlv_query" => tool_query(server, &args)?,
        "sqlv_exec" => tool_exec(server, &args)?,
        "sqlv_stats" => with_db(server, |db| {
            db.stats()
                .map(|s| serde_json::to_value(s).unwrap_or(json!(null)))
        })?,
        "sqlv_push_query" => tool_push_query(&args)?,
        "sqlv_push_open" => tool_push_open(&args)?,
        other => return Err(usage(&format!("unknown tool: {other}"))),
    };

    Ok(wrap_result(&result_json))
}

fn tool_open(server: &Server, args: &Value) -> Result<Value, JsonRpcError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_open: `path` is required"))?;
    let read_only = args
        .get("read_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let db = Db::open(
        &PathBuf::from(path),
        OpenOpts {
            read_only,
            timeout_ms: Some(5_000),
        },
    )
    .map_err(core_err)?;
    let meta = db.meta().map_err(core_err)?;
    *server.db.lock().unwrap_or_else(|p| p.into_inner()) = Some(db);
    Ok(serde_json::to_value(meta).unwrap_or(json!(null)))
}

fn tool_schema(server: &Server, args: &Value) -> Result<Value, JsonRpcError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_schema: `name` is required"))?;
    with_db(server, |db| {
        db.schema(name)
            .map(|s| serde_json::to_value(s).unwrap_or(json!(null)))
    })
}

fn tool_query(server: &Server, args: &Value) -> Result<Value, JsonRpcError> {
    let sql = args
        .get("sql")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_query: `sql` is required"))?;
    let params = parse_params(args.get("params"));
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(1000);
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(0);

    with_db(server, |db| {
        db.query(sql, &params, Page { limit, offset })
            .map(|r| serde_json::to_value(r).unwrap_or(json!(null)))
    })
}

fn tool_exec(server: &Server, args: &Value) -> Result<Value, JsonRpcError> {
    let sql = args
        .get("sql")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_exec: `sql` is required"))?;
    let confirm = args
        .get("confirm_destructive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !confirm {
        return Err(usage(
            "sqlv_exec refuses to run without confirm_destructive:true. Show the SQL to the user first.",
        ));
    }
    let params = parse_params(args.get("params"));
    with_db(server, |db| {
        db.exec(sql, &params)
            .map(|r| serde_json::to_value(r).unwrap_or(json!(null)))
    })
}

fn with_db(
    server: &Server,
    f: impl FnOnce(&Db) -> sqlv_core::Result<Value>,
) -> Result<Value, JsonRpcError> {
    let guard = server.db.lock().unwrap_or_else(|p| p.into_inner());
    let db = guard.as_ref().ok_or_else(|| JsonRpcError {
        code: -32000,
        message: "no database open — call sqlv_open first".into(),
    })?;
    f(db).map_err(core_err)
}

fn parse_params(v: Option<&Value>) -> Vec<SqlValue> {
    let Some(arr) = v.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter().map(SqlValue::from_json).collect()
}

fn core_err(e: sqlv_core::Error) -> JsonRpcError {
    let code = match e.code() {
        "not_found" => -32001,
        "readonly" => -32002,
        "sql" => -32003,
        "invalid" => -32602,
        _ => -32000,
    };
    JsonRpcError {
        code,
        message: e.to_string(),
    }
}

fn usage(msg: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: msg.into(),
    }
}

/// MCP wants tool results as `{content: [{type, text}], isError: bool}`.
/// We wrap the structured JSON as a text block so it's both human-readable
/// and machine-parseable by the host.
fn wrap_result(payload: &Value) -> Value {
    let text = serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".into());
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    })
}

// ---- Push bridge (agent → desktop over HTTP loopback) ----

#[derive(serde::Deserialize)]
struct InstanceInfo {
    port: u16,
    token: String,
}

fn tool_push_query(args: &Value) -> Result<Value, JsonRpcError> {
    let sql = args
        .get("sql")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_push_query: `sql` is required"))?;
    let body = json!({
        "sql": sql,
        "params": args.get("params").cloned().unwrap_or_else(|| json!([])),
        "limit": args.get("limit").and_then(|v| v.as_u64()).unwrap_or(1000),
        "offset": args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0),
        "mode": args.get("mode").and_then(|v| v.as_str()).unwrap_or("auto"),
    });
    http_post_to_desktop("/query", &body)
}

fn tool_push_open(args: &Value) -> Result<Value, JsonRpcError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| usage("sqlv_push_open: `path` is required"))?;
    let read_only = args
        .get("read_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let body = json!({ "path": path, "read_only": read_only });
    http_post_to_desktop("/open", &body)
}

fn http_post_to_desktop(path: &str, body: &Value) -> Result<Value, JsonRpcError> {
    let instances = list_instances();
    if instances.is_empty() {
        return Err(JsonRpcError {
            code: -32000,
            message: "no running desktop app found under ~/.sqlv/instances/. Start sqlv desktop and retry.".into(),
        });
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_millis(250))
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let mut last: Option<String> = None;
    for inst in &instances {
        let url = format!("http://127.0.0.1:{}{path}", inst.port);
        match agent
            .post(&url)
            .set("X-Sqlv-Token", &inst.token)
            .send_json(body.clone())
        {
            Ok(resp) => {
                let v: Value = resp.into_json().map_err(|e| JsonRpcError {
                    code: -32000,
                    message: format!("invalid response JSON: {e}"),
                })?;
                return Ok(v);
            }
            Err(ureq::Error::Status(_, resp)) => {
                let v: Value = resp.into_json().unwrap_or_else(|_| json!({}));
                // Return server's own error JSON so the agent can branch on code.
                return Ok(v);
            }
            Err(e) => {
                last = Some(e.to_string());
            }
        }
    }
    Err(JsonRpcError {
        code: -32000,
        message: format!(
            "could not reach desktop: {}",
            last.unwrap_or_else(|| "no response".into())
        ),
    })
}

fn list_instances() -> Vec<InstanceInfo> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let dir = home.join(".sqlv").join("instances");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&p) else {
            continue;
        };
        if let Ok(info) = serde_json::from_str::<InstanceInfo>(&body) {
            out.push(info);
        }
    }
    out
}
