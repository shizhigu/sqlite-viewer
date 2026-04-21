use std::path::PathBuf;
use std::sync::Arc;

use sqlv_core::{
    Db, DbMeta, ExecResult, IndexInfo, OpenOpts, Page, QueryResult, TableInfo, TableSchema, Value,
    ViewInfo,
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
    *state.current.lock().unwrap() = Some(db);
    Ok(meta)
}

#[tauri::command]
pub fn close_db(state: State<Arc<AppState>>) -> Res<()> {
    *state.current.lock().unwrap() = None;
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
    with_db(&state, |db| {
        db.query(&sql, &params, Page { limit, offset })
            .map_err(AppError::from)
    })
}

#[tauri::command]
pub fn run_exec(
    state: State<Arc<AppState>>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Res<ExecResult> {
    let params: Vec<Value> = params.iter().map(Value::from_json).collect();
    with_db(&state, |db| db.exec(&sql, &params).map_err(AppError::from))
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
    with_db(&state, |db| db.exec_many(&refs).map_err(AppError::from))
}

fn with_db<T>(
    state: &State<Arc<AppState>>,
    f: impl FnOnce(&Db) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let guard = state.current.lock().unwrap();
    let db = guard.as_ref().ok_or_else(AppError::not_open)?;
    f(db)
}
