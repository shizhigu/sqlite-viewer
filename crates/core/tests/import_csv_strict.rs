mod common;

use std::io::Write;

use sqlv_core::{CsvImportOpts, Db, OpenOpts, Page, Value};

fn write_csv(dir: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.path().join(name);
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(body.as_bytes()).unwrap();
    p
}

fn rw(file: &tempfile::NamedTempFile) -> Db {
    Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap()
}

#[test]
fn csv_import_coerces_numeric_columns() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(id INTEGER, price REAL, note TEXT);", &[])
        .unwrap();
    let csv = write_csv(&dir, "t.csv", "id,price,note\n1,9.99,hello\n2,12.5,world\n");
    db.import_csv(&csv, "t", CsvImportOpts::default()).unwrap();

    // Verify storage classes match declared affinity, not raw TEXT.
    let r = db
        .query(
            "SELECT typeof(id), typeof(price), typeof(note) FROM t LIMIT 1",
            &[],
            Page::default(),
        )
        .unwrap();
    assert_eq!(r.rows[0][0], Value::Text("integer".into()));
    assert_eq!(r.rows[0][1], Value::Text("real".into()));
    assert_eq!(r.rows[0][2], Value::Text("text".into()));
}

#[test]
fn csv_import_succeeds_on_strict_table() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec(
        "CREATE TABLE s(id INTEGER, amt REAL, label TEXT) STRICT;",
        &[],
    )
    .unwrap();
    let csv = write_csv(&dir, "s.csv", "id,amt,label\n7,3.25,seven\n8,4.75,eight\n");
    let res = db.import_csv(&csv, "s", CsvImportOpts::default()).unwrap();
    assert_eq!(res.rows_inserted, 2);
}

#[test]
fn csv_import_unparseable_int_falls_back_to_text() {
    // For non-STRICT tables this still succeeds because SQLite's type
    // affinity stores the raw text.
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(id INTEGER);", &[]).unwrap();
    let csv = write_csv(&dir, "t.csv", "id\n1\nnot-an-int\n3\n");
    let res = db.import_csv(&csv, "t", CsvImportOpts::default()).unwrap();
    assert_eq!(res.rows_inserted, 3);
    let r = db
        .query(
            "SELECT typeof(id), id FROM t ORDER BY rowid",
            &[],
            Page::default(),
        )
        .unwrap();
    assert_eq!(r.rows[0][0], Value::Text("integer".into()));
    assert_eq!(r.rows[1][0], Value::Text("text".into()));
    assert_eq!(r.rows[1][1], Value::Text("not-an-int".into()));
}

#[test]
fn csv_import_into_strict_rejects_bad_type() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE s(id INTEGER) STRICT;", &[]).unwrap();
    let csv = write_csv(&dir, "s.csv", "id\n1\nnot-an-int\n");
    // STRICT tables reject the TEXT fallback — the whole file rolls back.
    let err = db
        .import_csv(&csv, "s", CsvImportOpts::default())
        .unwrap_err();
    assert_eq!(err.code(), "sql");
    let q = db
        .query("SELECT COUNT(*) FROM s", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(0));
}
