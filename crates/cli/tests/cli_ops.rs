mod common;

use serde_json::json;

#[test]
fn stats_returns_per_table_row_counts() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["stats", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert!(v["size_bytes"].as_u64().unwrap() > 0);
    let tables = v["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 2);
    let rc: std::collections::HashMap<_, _> = tables
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap().to_string(),
                t["row_count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(rc["artists"], 3);
    assert_eq!(rc["albums"], 4);
}

#[test]
fn pragma_reads_user_version() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["pragma", "--db"]).arg(&db).arg("user_version");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["name"], "user_version");
    assert_eq!(v["values"], json!([["0"]]));
}

#[test]
fn pragma_set_without_write_is_usage_error() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["pragma", "--db"])
        .arg(&db)
        .arg("user_version")
        .arg("5");
    let out = cmd.assert().failure().code(2).get_output().clone();
    let err = common::parse_json_stderr(&out);
    assert_eq!(err["error"]["code"], "usage");
}

#[test]
fn pragma_set_with_write_updates_value() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["pragma", "--db"])
        .arg(&db)
        .arg("user_version")
        .arg("7")
        .arg("--write");
    let out = cmd.assert().success().get_output().clone();
    let v = common::parse_json_stdout(&out);
    assert_eq!(v["values"], json!([["7"]]));
}

#[test]
fn pragma_injection_attempt_rejected_as_invalid() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["pragma", "--db"])
        .arg(&db)
        .arg("user_version; DROP TABLE artists");
    let out = cmd.assert().failure().get_output().clone();
    let err = common::parse_json_stderr(&out);
    // sqlv_core returns Invalid; CLI maps to code "invalid", exit 1.
    assert_eq!(err["error"]["code"], "invalid");
}

#[test]
fn dump_full_emits_schema_and_data() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    let sql = String::from_utf8(out.stdout).unwrap();
    assert!(sql.contains("BEGIN TRANSACTION;"));
    assert!(sql.contains("CREATE TABLE artists"));
    assert!(sql.contains("INSERT INTO \"artists\""));
    assert!(sql.contains("'Miles Davis'"));
    assert!(sql.trim_end().ends_with("COMMIT;"));
}

#[test]
fn dump_schema_only_has_no_inserts() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"]).arg(&db).arg("--schema-only");
    let out = cmd.assert().success().get_output().clone();
    let sql = String::from_utf8(out.stdout).unwrap();
    assert!(sql.contains("CREATE TABLE"));
    assert!(!sql.contains("INSERT INTO"));
}

#[test]
fn dump_data_only_has_no_creates() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"]).arg(&db).arg("--data-only");
    let out = cmd.assert().success().get_output().clone();
    let sql = String::from_utf8(out.stdout).unwrap();
    assert!(!sql.contains("CREATE TABLE"));
    assert!(sql.contains("INSERT INTO"));
}

#[test]
fn dump_schema_and_data_flags_are_mutually_exclusive() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"])
        .arg(&db)
        .arg("--schema-only")
        .arg("--data-only");
    // clap returns exit code 2 for conflicting args
    cmd.assert().failure().code(2);
}

#[test]
fn dump_filter_by_table_excludes_others() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"])
        .arg(&db)
        .args(["--table", "artists"]);
    let out = cmd.assert().success().get_output().clone();
    let sql = String::from_utf8(out.stdout).unwrap();
    assert!(sql.contains("CREATE TABLE artists"));
    assert!(!sql.contains("CREATE TABLE albums"));
    assert!(sql.contains("INSERT INTO \"artists\""));
    assert!(!sql.contains("INSERT INTO \"albums\""));
}

#[test]
fn dump_output_is_well_formed_text() {
    let (mut cmd, _dir, db) = common::make_catalogue();
    cmd.args(["dump", "--db"]).arg(&db);
    let out = cmd.assert().success().get_output().clone();
    assert!(out.stdout.ends_with(b"\n"));
}
