mod common;

#[test]
fn help_prints_and_exits_zero() {
    let (mut cmd, _dir, _db) = common::make_empty();
    cmd.arg("--help");
    let out = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Agent-friendly SQLite client"));
    assert!(stdout.contains("tables"));
    assert!(stdout.contains("query"));
    assert!(stdout.contains("exec"));
}

#[test]
fn version_prints_something() {
    let (mut cmd, _dir, _db) = common::make_empty();
    cmd.arg("--version");
    let out = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("sqlv "));
}

#[test]
fn missing_db_flag_is_usage_error() {
    let (mut cmd, _dir, _db) = common::make_empty();
    cmd.arg("tables");
    cmd.assert().failure().code(2);
}

#[test]
fn no_subcommand_is_usage_error() {
    let (mut cmd, _dir, _db) = common::make_empty();
    cmd.assert().failure().code(2);
}
