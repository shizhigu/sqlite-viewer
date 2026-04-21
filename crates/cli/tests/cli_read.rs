mod common;

use predicates::prelude::*;
use serde_json::json;

#[test]
fn open_prints_meta_json() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["open", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["encoding"], "UTF-8");
    assert!(v["page_size"].as_i64().unwrap() >= 512);
    assert_eq!(v["read_only"], json!(true));
    assert!(!v["sqlite_library_version"].as_str().unwrap().is_empty());
}

#[test]
fn open_on_missing_file_errors_with_sql_code() {
    let (mut cmd, dir, _db) = common::make_catalogue();
    let missing = dir.path().join("nope.sqlite");
    cmd.args(["open", "--db"]).arg(&missing);
    let out = cmd.assert().failure().code(5).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "sql");
}

#[test]
fn tables_lists_user_tables_sorted_with_row_counts() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["tables", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "albums");
    assert_eq!(arr[1]["name"], "artists");
    assert_eq!(arr[0]["row_count"], 4);
    assert_eq!(arr[1]["row_count"], 3);
    for t in arr {
        assert_eq!(t["kind"], "table");
    }
}

#[test]
fn tables_on_empty_db_returns_empty_array() {
    let (mut cmd, _dir, db) = common::make_empty();
    cmd.args(["tables", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v, json!([]));
}

#[test]
fn views_returns_the_single_view() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["views", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "recent_albums");
    assert!(arr[0]["sql"].as_str().unwrap().contains("year >= 2000"));
}

#[test]
fn indexes_all_tables() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["indexes", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let arr = v.as_array().unwrap();
    assert!(arr.iter().any(|i| i["name"] == "idx_albums_artist"));
}

#[test]
fn indexes_filtered_by_table() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["indexes", "--db"])
        .arg(&db)
        .args(["--table", "albums"]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let arr = v.as_array().unwrap();
    assert!(arr.iter().all(|i| i["table"] == "albums"));
}

#[test]
fn indexes_missing_table_exits_with_not_found_code() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["indexes", "--db"])
        .arg(&db)
        .args(["--table", "nope"]);
    let out = cmd.assert().failure().code(3).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "not_found");
}

#[test]
fn schema_without_arg_lists_every_table() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["schema", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let names: Vec<&str> = arr.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"albums"));
    assert!(names.contains(&"artists"));
}

#[test]
fn schema_with_table_returns_columns_pk_fk() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["schema", "--db"]).arg(&db).arg("albums");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["name"], "albums");
    assert_eq!(v["kind"], "table");
    let cols = v["columns"].as_array().unwrap();
    assert_eq!(cols.len(), 4);
    assert!(cols.iter().any(|c| c["name"] == "id" && c["pk"] == 1));
    let fks = v["foreign_keys"].as_array().unwrap();
    assert_eq!(fks.len(), 1);
    assert_eq!(fks[0]["table"], "artists");
    assert_eq!(fks[0]["on_delete"], "CASCADE");
}

#[test]
fn schema_missing_table_is_not_found() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["schema", "--db"]).arg(&db).arg("no_such");
    let out = cmd.assert().failure().code(3).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "not_found");
}

#[test]
fn stderr_error_payload_has_code_and_message_fields() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["schema", "--db"]).arg(&db).arg("no_such");
    let out = cmd.assert().failure().get_output().clone();
    let stderr = std::str::from_utf8(&out.stderr).unwrap();
    assert!(predicates::str::contains("\"code\"").eval(stderr));
    assert!(predicates::str::contains("\"message\"").eval(stderr));
}
