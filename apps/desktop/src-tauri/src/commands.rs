use std::path::PathBuf;
use std::sync::Arc;

use sqlv_core::{
    ActivityEntry, ActivityLog, ActivityQuery, ActivityRecord, Db, DbMeta, ExecResult, IndexInfo,
    OpenOpts, Page, QueryResult, SchemaInfo, TableInfo, TableSchema, TriggerInfo, Value, ViewInfo,
};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

type Res<T> = Result<T, AppError>;

#[tauri::command]
pub fn ping() -> Res<String> {
    Ok(format!(
        "sqlv-desktop v{} (sqlv-core wired)",
        env!("CARGO_PKG_VERSION")
    ))
}

#[tauri::command]
pub fn open_db(state: State<Arc<AppState>>, path: String, read_only: bool) -> Res<DbMeta> {
    let db = Db::open(
        &PathBuf::from(&path),
        OpenOpts {
            read_only,
            timeout_ms: Some(5_000),
        },
    )?;
    let meta = db.meta()?;
    *state.cancel.lock().unwrap() = Some(db.cancel_handle());
    *state.current.lock().unwrap() = Some(db);
    log_activity(&state, "ui", "open", None, Some(&path), None, None, None);
    Ok(meta)
}

#[tauri::command]
pub fn close_db(state: State<Arc<AppState>>) -> Res<()> {
    *state.current.lock().unwrap() = None;
    *state.cancel.lock().unwrap() = None;
    Ok(())
}

/// Signal a cancel to the running query. Harmless no-op if no query is
/// currently executing on the Db.
#[tauri::command]
pub fn cancel_query(state: State<Arc<AppState>>) -> Res<()> {
    if let Some(h) = state.cancel.lock().unwrap().as_ref() {
        h.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn list_tables(state: State<Arc<AppState>>) -> Res<Vec<TableInfo>> {
    with_db(&state, |db| db.tables().map_err(AppError::from))
}

#[tauri::command]
pub fn list_views(state: State<Arc<AppState>>) -> Res<Vec<ViewInfo>> {
    with_db(&state, |db| db.views().map_err(AppError::from))
}

#[tauri::command]
pub fn list_schemas(state: State<Arc<AppState>>) -> Res<Vec<SchemaInfo>> {
    with_db(&state, |db| db.schemas().map_err(AppError::from))
}

#[tauri::command]
pub fn list_tables_in_schema(state: State<Arc<AppState>>, schema: String) -> Res<Vec<TableInfo>> {
    with_db(&state, |db| {
        db.tables_in_schema(&schema).map_err(AppError::from)
    })
}

#[tauri::command]
pub fn list_triggers(state: State<Arc<AppState>>) -> Res<Vec<TriggerInfo>> {
    with_db(&state, |db| db.triggers().map_err(AppError::from))
}

#[allow(dead_code)]
pub fn list_indexes(state: State<Arc<AppState>>, table: Option<String>) -> Res<Vec<IndexInfo>> {
    with_db(&state, |db| {
        db.indexes(table.as_deref()).map_err(AppError::from)
    })
}

#[tauri::command]
pub fn describe_table(state: State<Arc<AppState>>, name: String) -> Res<TableSchema> {
    with_db(&state, |db| db.schema(&name).map_err(AppError::from))
}

#[tauri::command]
pub fn run_query(
    state: State<Arc<AppState>>,
    sql: String,
    params: Vec<serde_json::Value>,
    limit: u32,
    offset: u32,
) -> Res<QueryResult> {
    let params: Vec<Value> = params.iter().map(Value::from_json).collect();
    let db_path = db_path(&state);
    let res = with_db(&state, |db| {
        db.query(&sql, &params, Page { limit, offset })
            .map_err(AppError::from)
    });
    match &res {
        Ok(r) => log_activity(
            &state,
            "ui",
            "query",
            Some(&sql),
            db_path.as_deref(),
            Some(r.elapsed_ms as i64),
            Some(r.rows.len() as i64),
            None,
        ),
        Err(e) => log_activity(
            &state,
            "ui",
            "query",
            Some(&sql),
            db_path.as_deref(),
            None,
            None,
            Some(e),
        ),
    }
    res
}

#[tauri::command]
pub fn run_exec(
    state: State<Arc<AppState>>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Res<ExecResult> {
    let params: Vec<Value> = params.iter().map(Value::from_json).collect();
    let db_path = db_path(&state);
    let res = with_db(&state, |db| db.exec(&sql, &params).map_err(AppError::from));
    match &res {
        Ok(r) => log_activity(
            &state,
            "ui",
            "exec",
            Some(&sql),
            db_path.as_deref(),
            Some(r.elapsed_ms as i64),
            Some(r.rows_affected as i64),
            None,
        ),
        Err(e) => log_activity(
            &state,
            "ui",
            "exec",
            Some(&sql),
            db_path.as_deref(),
            None,
            None,
            Some(e),
        ),
    }
    res
}

/// Execute a batch of statements inside one transaction — all-or-nothing.
/// The frontend uses this for multi-row deletes so a failure mid-batch
/// doesn't leave the table partially mutated.
#[tauri::command]
pub fn run_exec_many(
    state: State<Arc<AppState>>,
    statements: Vec<(String, Vec<serde_json::Value>)>,
) -> Res<ExecResult> {
    let converted: Vec<(String, Vec<Value>)> = statements
        .into_iter()
        .map(|(sql, params)| (sql, params.iter().map(Value::from_json).collect()))
        .collect();
    let refs: Vec<(&str, &[Value])> = converted
        .iter()
        .map(|(sql, params)| (sql.as_str(), params.as_slice()))
        .collect();
    let db_path = db_path(&state);
    let res = with_db(&state, |db| db.exec_many(&refs).map_err(AppError::from));
    let joined = converted
        .iter()
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    match &res {
        Ok(r) => log_activity(
            &state,
            "ui",
            "exec_many",
            Some(&joined),
            db_path.as_deref(),
            Some(r.elapsed_ms as i64),
            Some(r.rows_affected as i64),
            None,
        ),
        Err(e) => log_activity(
            &state,
            "ui",
            "exec_many",
            Some(&joined),
            db_path.as_deref(),
            None,
            None,
            Some(e),
        ),
    }
    res
}

/// Run COUNT(*) with an optional WHERE clause against the given table.
/// Used by the live-count ghost in the query editor.
#[tauri::command]
pub fn count_rows(
    state: State<Arc<AppState>>,
    table: String,
    where_clause: Option<String>,
    params: Vec<serde_json::Value>,
) -> Res<i64> {
    let params: Vec<Value> = params.iter().map(Value::from_json).collect();
    with_db(&state, |db| {
        let quoted = quote_ident(&table);
        let sql = match where_clause.as_deref() {
            Some(w) if !w.trim().is_empty() => {
                format!("SELECT COUNT(*) FROM {quoted} WHERE {w}")
            }
            _ => format!("SELECT COUNT(*) FROM {quoted}"),
        };
        let r = db
            .query(
                &sql,
                &params,
                Page {
                    limit: 1,
                    offset: 0,
                },
            )
            .map_err(AppError::from)?;
        match r.rows.first().and_then(|row| row.first()) {
            Some(Value::Integer(n)) => Ok(*n),
            _ => Ok(0),
        }
    })
}

// ------------------------------------------------------------------
// Persistent activity log commands. The log lives at ~/.sqlv/activity.db
// and is shared with the CLI's `sqlv history` subcommand.
// ------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct ActivityQueryResult {
    pub records: Vec<ActivityRecord>,
}

#[tauri::command]
pub fn activity_query(
    state: State<Arc<AppState>>,
    grep: Option<String>,
    since_ms: Option<i64>,
    db_path: Option<String>,
    source: Option<String>,
    limit: Option<u32>,
) -> Res<ActivityQueryResult> {
    let q = ActivityQuery {
        grep,
        since_ms,
        db_path,
        source,
        limit: limit.unwrap_or(500),
    };
    let records = with_activity(&state, |log| log.query(&q).map_err(AppError::from))?;
    Ok(ActivityQueryResult { records })
}

#[tauri::command]
pub fn activity_prune(state: State<Arc<AppState>>, cutoff_ms: i64) -> Res<u64> {
    with_activity(&state, |log| {
        log.prune_before(cutoff_ms).map_err(AppError::from)
    })
}

// ------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------

fn db_path(state: &State<Arc<AppState>>) -> Option<String> {
    state
        .current
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|db| db.path().display().to_string()))
}

/// Best-effort append to the persistent activity log. Logging failures are
/// swallowed — the log is observational, not load-bearing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn log_activity(
    state: &State<Arc<AppState>>,
    source: &str,
    kind: &str,
    sql: Option<&str>,
    db_path: Option<&str>,
    elapsed_ms: Option<i64>,
    rows: Option<i64>,
    error: Option<&AppError>,
) {
    let mut entry = ActivityEntry::now(source, kind);
    entry.sql = sql.map(|s| s.to_string());
    entry.db_path = db_path.map(|s| s.to_string());
    entry.elapsed_ms = elapsed_ms;
    entry.rows = rows;
    if let Some(e) = error {
        entry.error_code = Some(e.code.clone());
        entry.error_message = Some(e.message.clone());
    }

    let mut guard = match state.activity.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if guard.is_none() {
        if let Ok(log) = ActivityLog::open_default() {
            *guard = Some(log);
        }
    }
    if let Some(log) = guard.as_ref() {
        let _ = log.append(&entry);
    }
}

fn with_activity<T>(
    state: &State<Arc<AppState>>,
    f: impl FnOnce(&ActivityLog) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let mut guard = match state.activity.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if guard.is_none() {
        let log = ActivityLog::open_default().map_err(AppError::from)?;
        *guard = Some(log);
    }
    f(guard.as_ref().expect("activity log just opened"))
}

fn with_db<T>(
    state: &State<Arc<AppState>>,
    f: impl FnOnce(&Db) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let guard = state.current.lock().unwrap();
    let db = guard.as_ref().ok_or_else(AppError::not_open)?;
    f(db)
}

fn quote_ident(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        if ch == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(ch);
        }
    }
    out.push('"');
    out
}
