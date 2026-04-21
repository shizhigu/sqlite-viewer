mod common;

use std::io::Write;

use sqlv_core::{CsvImportOpts, Db, OpenOpts, Page, Value};

fn write_csv(dir: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.path().join(name);
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(body.as_bytes()).unwrap();
    p
}

#[test]
fn import_csv_with_header_and_default_opts() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE people(name TEXT, age TEXT, email TEXT);", &[])
        .unwrap();
    let csv = write_csv(
        &dir,
        "p.csv",
        "name,age,email\nAlice,30,alice@x\nBob,,bob@x\n",
    );
    let res = db
        .import_csv(&csv, "people", CsvImportOpts::default())
        .unwrap();
    assert_eq!(res.rows_inserted, 2);

    let q = db
        .query(
            "SELECT name, age, email FROM people ORDER BY rowid",
            &[],
            Page::default(),
        )
        .unwrap();
    // Default opts = null_token: None. Empty string "" is stored as TEXT "".
    assert_eq!(q.rows[1][1], Value::Text(String::new()));
}

#[test]
fn import_csv_null_token_empty_string_maps_to_null() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE p(name TEXT, age TEXT);", &[])
        .unwrap();
    let csv = write_csv(&dir, "p.csv", "name,age\nAlice,30\nBob,\n");

    let res = db
        .import_csv(
            &csv,
            "p",
            CsvImportOpts {
                null_token: Some("".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(res.rows_inserted, 2);

    let q = db
        .query("SELECT age FROM p ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Text("30".into()));
    assert_eq!(q.rows[1][0], Value::Null, "empty field became NULL");
}

#[test]
fn import_csv_null_token_literal_string_null_maps_to_null() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(x TEXT);", &[]).unwrap();
    // Some CSVs literally contain "NULL" where they mean NULL.
    let csv = write_csv(&dir, "t.csv", "x\nhello\nNULL\n\"\"\n");

    db.import_csv(
        &csv,
        "t",
        CsvImportOpts {
            null_token: Some("NULL".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let q = db
        .query("SELECT x FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Text("hello".into()));
    assert_eq!(
        q.rows[1][0],
        Value::Null,
        "the literal string NULL became NULL"
    );
    // The quoted empty third row remains TEXT "" because the token is "NULL", not "".
    assert_eq!(q.rows[2][0], Value::Text(String::new()));
}

#[test]
fn import_csv_rolls_back_entire_file_on_any_row_error() {
    let dir = tempfile::tempdir().unwrap();
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
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL);",
        &[],
    )
    .unwrap();
    // Second row violates NOT NULL (field becomes NULL via the token).
    let csv = write_csv(&dir, "t.csv", "id,v\n1,a\n2,\n3,c\n");
    let err = db
        .import_csv(
            &csv,
            "t",
            CsvImportOpts {
                null_token: Some("".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(err.code(), "sql");
    // Nothing committed.
    let q = db
        .query("SELECT COUNT(*) FROM t", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(0));
}
