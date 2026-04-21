mod common;

use serde_json::json;

#[test]
fn query_returns_rows_and_columns() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT id, name FROM artists ORDER BY id");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["columns"], json!(["id", "name"]));
    let rows = v["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], json!([1, "Miles Davis"]));
}

#[test]
fn query_with_params() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT COUNT(*) FROM albums WHERE year >= ?1")
        .args(["-p", "2000"]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows"][0][0], json!(1));
}

#[test]
fn query_with_multiple_params_of_different_types() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT title FROM albums WHERE artist_id = ?1 AND title = ?2")
        .args(["-p", "2"])
        .args(["-p", "\"Vespertine\""]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows"][0][0], json!("Vespertine"));
}

#[test]
fn query_param_bare_identifier_is_usage_error() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT ?1")
        .args(["-p", "not-valid-json-bareword"]);
    let out = cmd.assert().failure().code(2).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "usage");
}

#[test]
fn query_respects_limit_and_marks_truncated() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT id FROM albums ORDER BY id")
        .args(["--limit", "2"]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows"].as_array().unwrap().len(), 2);
    assert_eq!(v["truncated"], json!(true));
}

#[test]
fn query_with_offset() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT id FROM albums ORDER BY id")
        .args(["--limit", "2"])
        .args(["--offset", "2"]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    let ids: Vec<i64> = v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![3, 4]);
}

#[test]
fn query_syntax_error_exit_code_5() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELEKT * FROM artists");
    let out = cmd.assert().failure().code(5).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "sql");
}

#[test]
fn query_null_round_trip() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"]).arg(&db).arg("SELECT NULL, 'x'");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows"][0][0], json!(null));
    assert_eq!(v["rows"][0][1], json!("x"));
}

#[test]
fn query_blob_emits_base64_tagged_object() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT x'deadbeef'");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows"][0][0]["$blob_base64"], json!("3q2+7w=="));
}

#[test]
fn query_elapsed_ms_is_number() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["query", "--db"]).arg(&db).arg("SELECT 1");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert!(v["elapsed_ms"].is_number());
}

#[test]
fn exec_without_write_flag_is_refused() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["exec", "--db"])
        .arg(&db)
        .arg("INSERT INTO artists(name) VALUES ('x')");
    let out = cmd.assert().failure().code(2).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "usage");
    let msg = err["error"]["message"].as_str().unwrap();
    assert!(msg.contains("--write"));
}

#[test]
fn exec_with_write_performs_insert_and_reports_row_count() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["exec", "--db"])
        .arg(&db)
        .arg("--write")
        .arg("INSERT INTO artists(name) VALUES (?1)")
        .args(["-p", "\"Laurie Anderson\""]);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["rows_affected"], json!(1));
    assert!(v["last_insert_rowid"].as_i64().unwrap() > 0);

    // And verify via query
    let (mut cmd2, _dir2, _db2) = (
        assert_cmd::Command::cargo_bin("sqlv").unwrap(),
        _dir,
        db.clone(),
    );
    cmd2.args(["query", "--db"])
        .arg(&db)
        .arg("SELECT COUNT(*) FROM artists");
    let v2 = common::parse_json_stdout(&cmd2.assert().success().get_output().clone());
    assert_eq!(v2["rows"][0][0], json!(4));
}

#[test]
fn exec_write_sql_error_exit_code_5() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["exec", "--db"])
        .arg(&db)
        .arg("--write")
        .arg("INSERT INTO albums(title, artist_id, year) VALUES ('bad', 999, 2020)");
    let out = cmd.assert().failure().code(5).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "sql");
}

#[test]
fn exec_write_constraint_notnull() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["exec", "--db"])
        .arg(&db)
        .arg("--write")
        .arg("INSERT INTO artists(name) VALUES (NULL)");
    let out = cmd.assert().failure().code(5).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "sql");
}

#[test]
fn stdout_json_always_ends_with_newline() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["tables", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    assert!(out.stdout.ends_with(b"\n"));
}
