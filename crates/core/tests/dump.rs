mod common;

use sqlv_core::{Db, DumpFilter, OpenOpts};

#[test]
fn dump_full_contains_schema_and_data() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let sql = db.dump(DumpFilter::default()).unwrap();

    assert!(sql.starts_with("PRAGMA foreign_keys = OFF;"));
    assert!(sql.contains("BEGIN TRANSACTION;"));
    assert!(sql.trim_end().ends_with("COMMIT;"));
    assert!(sql.contains("CREATE TABLE artists"));
    assert!(sql.contains("CREATE TABLE albums"));
    assert!(sql.contains("CREATE INDEX idx_albums_artist"));
    assert!(sql.contains("CREATE VIEW recent_albums"));
    assert!(sql.contains("INSERT INTO \"artists\""));
    assert!(sql.contains("'Miles Davis'"));
    assert!(sql.contains("'Björk'"));
}

#[test]
fn dump_schema_only_has_no_inserts() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let sql = db
        .dump(DumpFilter {
            schema: true,
            data: false,
            only_tables: None,
        })
        .unwrap();
    assert!(sql.contains("CREATE TABLE"));
    assert!(!sql.contains("INSERT INTO"));
}

#[test]
fn dump_data_only_has_no_creates() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let sql = db
        .dump(DumpFilter {
            schema: false,
            data: true,
            only_tables: None,
        })
        .unwrap();
    assert!(!sql.contains("CREATE TABLE"));
    assert!(sql.contains("INSERT INTO"));
}

#[test]
fn dump_filter_restricts_to_named_tables() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let only = vec!["artists".to_string()];
    let sql = db
        .dump(DumpFilter {
            schema: true,
            data: true,
            only_tables: Some(&only),
        })
        .unwrap();
    assert!(sql.contains("CREATE TABLE artists"));
    assert!(!sql.contains("CREATE TABLE albums"));
    assert!(sql.contains("INSERT INTO \"artists\""));
    assert!(!sql.contains("INSERT INTO \"albums\""));
}

#[test]
fn dump_handles_null_integer_text_real_blob_literals() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec(
        "CREATE TABLE mixed (a INTEGER, b REAL, c TEXT, d BLOB);",
        &[],
    )
    .unwrap();
    db.exec(
        "INSERT INTO mixed VALUES (NULL, 3.5, 'hi ''there''', x'DEADBEEF');",
        &[],
    )
    .unwrap();
    let sql = db.dump(DumpFilter::default()).unwrap();
    assert!(sql.contains("(NULL, 3.5, 'hi ''there''', X'DEADBEEF')"));
}

#[test]
fn dump_on_empty_db_still_produces_valid_envelope() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let sql = db.dump(DumpFilter::default()).unwrap();
    assert!(sql.contains("BEGIN TRANSACTION;"));
    assert!(sql.trim_end().ends_with("COMMIT;"));
    // No tables → no CREATE / INSERT lines at all.
    assert!(!sql.contains("CREATE TABLE"));
    assert!(!sql.contains("INSERT INTO"));
}

#[test]
fn dump_output_is_replayable_into_fresh_database() {
    let src_file = common::make_catalogue();
    let src = Db::open(src_file.path(), OpenOpts::default()).unwrap();
    let sql = src.dump(DumpFilter::default()).unwrap();

    let dest_file = common::make_empty();
    let dest = Db::open(
        dest_file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    // execute_batch to replay the whole dump. This is the real compatibility
    // test: the dump must be re-ingestible.
    rusqlite_exec_batch(&dest, &sql);

    let count: i64 = rusqlite_query_scalar(&dest, "SELECT COUNT(*) FROM artists");
    assert_eq!(count, 3);
    let count: i64 = rusqlite_query_scalar(&dest, "SELECT COUNT(*) FROM albums");
    assert_eq!(count, 4);
}

// ---- helpers that reach past the public API for replay assertions ----

fn rusqlite_exec_batch(_db: &Db, sql: &str) {
    // Parse & run the dump via a fresh rusqlite connection at the same file.
    // We avoid exposing execute_batch in the public API to keep the surface
    // agent-shaped; for tests we just go through rusqlite directly.
    let path = _db_path(_db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(sql).unwrap();
}

fn rusqlite_query_scalar(_db: &Db, sql: &str) -> i64 {
    let path = _db_path(_db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn _db_path(db: &Db) -> std::path::PathBuf {
    db.path().to_path_buf()
}
