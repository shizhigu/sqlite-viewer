mod common;

use std::io::Write;

use sqlv_core::{Db, JsonFormat, OpenOpts, Page, Value};

fn write_file(dir: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
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
fn import_json_array_inserts_rows_using_first_object_keys() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(id INTEGER, name TEXT, score REAL);", &[])
        .unwrap();
    let body = r#"[
        {"id": 1, "name": "Alice", "score": 9.5},
        {"id": 2, "name": "Bob", "score": 7.25}
    ]"#;
    let p = write_file(&dir, "t.json", body);

    let res = db.import_json(&p, "t", JsonFormat::Array).unwrap();
    assert_eq!(res.rows_inserted, 2);
    assert_eq!(res.columns, vec!["id", "name", "score"]);

    let q = db
        .query(
            "SELECT id, name, score FROM t ORDER BY id",
            &[],
            Page::default(),
        )
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(1));
    assert_eq!(q.rows[0][1], Value::Text("Alice".into()));
    assert_eq!(q.rows[0][2], Value::Real(9.5));
}

#[test]
fn import_jsonl_handles_blank_lines_and_ndjson() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(id INTEGER, name TEXT);", &[])
        .unwrap();
    let body = "{\"id\":1,\"name\":\"Alice\"}\n\n{\"id\":2,\"name\":\"Bob\"}\n";
    let p = write_file(&dir, "t.jsonl", body);

    let res = db.import_json(&p, "t", JsonFormat::Lines).unwrap();
    assert_eq!(res.rows_inserted, 2);
}

#[test]
fn import_json_missing_key_inserts_null() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(a TEXT, b TEXT);", &[]).unwrap();
    let body = r#"[{"a":"one","b":"first"},{"a":"two"}]"#;
    let p = write_file(&dir, "t.json", body);
    db.import_json(&p, "t", JsonFormat::Array).unwrap();

    let q = db
        .query("SELECT a, b FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[1][0], Value::Text("two".into()));
    assert_eq!(q.rows[1][1], Value::Null);
}

#[test]
fn import_json_extra_key_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(a INTEGER);", &[]).unwrap();
    let body = r#"[{"a":1},{"a":2,"extra":"ignore me"}]"#;
    let p = write_file(&dir, "t.json", body);
    let res = db.import_json(&p, "t", JsonFormat::Array).unwrap();
    assert_eq!(res.rows_inserted, 2);
    assert_eq!(res.columns, vec!["a"]);
}

#[test]
fn import_json_rolls_back_on_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    db.exec("CREATE TABLE t(a TEXT);", &[]).unwrap();
    let body = "{\"a\":\"ok\"}\n{not valid json\n";
    let p = write_file(&dir, "t.jsonl", body);
    let err = db.import_json(&p, "t", JsonFormat::Lines).unwrap_err();
    assert_eq!(err.code(), "invalid");
    let q = db
        .query("SELECT COUNT(*) FROM t", &[], Page::default())
        .unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(0));
}

#[test]
fn import_json_missing_table_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::make_empty();
    let db = rw(&file);
    let p = write_file(&dir, "t.json", "[{\"a\":1}]");
    let err = db.import_json(&p, "nope", JsonFormat::Array).unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[test]
fn guess_json_format_from_extension() {
    assert!(matches!(
        sqlv_core::guess_json_format(std::path::Path::new("a.json")),
        Some(JsonFormat::Array)
    ));
    assert!(matches!(
        sqlv_core::guess_json_format(std::path::Path::new("a.jsonl")),
        Some(JsonFormat::Lines)
    ));
    assert!(matches!(
        sqlv_core::guess_json_format(std::path::Path::new("a.ndjson")),
        Some(JsonFormat::Lines)
    ));
    assert!(sqlv_core::guess_json_format(std::path::Path::new("a.csv")).is_none());
}
