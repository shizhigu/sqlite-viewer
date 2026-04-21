//! Integration-test fixtures for the sqlv CLI.
//!
//! Each test builds a catalogue DB in a temp dir, then shells out to the
//! compiled `sqlv` binary via `assert_cmd`.

#![allow(dead_code)]

use assert_cmd::Command;
use tempfile::TempDir;

/// Returns (binary, temp_dir, db_path). Keep the `TempDir` alive for the test
/// — dropping it removes the DB.
pub fn make_catalogue() -> (Command, TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("catalogue.sqlite");

    // Seed with a direct rusqlite connection via sqlv-core. This keeps the
    // CLI tests from depending on `sqlv exec --write` to build fixtures
    // (which we also want to test independently).
    let db = sqlv_core::Db::open(
        &db_path,
        sqlv_core::OpenOpts {
            read_only: false,
            timeout_ms: Some(1_000),
        },
    )
    .unwrap();
    db.exec(
        "CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
        &[],
    )
    .unwrap();
    db.exec(
        "CREATE TABLE albums (\
           id INTEGER PRIMARY KEY,\
           title TEXT NOT NULL,\
           artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,\
           year INTEGER\
         );",
        &[],
    )
    .unwrap();
    db.exec("CREATE INDEX idx_albums_artist ON albums(artist_id);", &[])
        .unwrap();
    db.exec(
        "CREATE VIEW recent_albums AS SELECT * FROM albums WHERE year >= 2000;",
        &[],
    )
    .unwrap();
    db.exec(
        "INSERT INTO artists (name) VALUES ('Miles Davis'),('Björk'),('Aphex Twin');",
        &[],
    )
    .unwrap();
    db.exec(
        "INSERT INTO albums (title, artist_id, year) VALUES \
         ('Kind of Blue', 1, 1959), \
         ('Homogenic', 2, 1997), \
         ('Selected Ambient Works 85–92', 3, 1992), \
         ('Vespertine', 2, 2001);",
        &[],
    )
    .unwrap();

    let cmd = Command::cargo_bin("sqlv").unwrap();
    (cmd, dir, db_path)
}

/// Empty DB — file exists but no tables.
pub fn make_empty() -> (Command, TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("empty.sqlite");
    let _ = sqlv_core::Db::open(
        &db_path,
        sqlv_core::OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let cmd = Command::cargo_bin("sqlv").unwrap();
    (cmd, dir, db_path)
}

/// Decode stdout as a JSON value. Panics with a helpful message if not parseable.
pub fn parse_json_stdout(out: &std::process::Output) -> serde_json::Value {
    let s = std::str::from_utf8(&out.stdout).expect("stdout not utf-8");
    serde_json::from_str(s).unwrap_or_else(|e| panic!("stdout not JSON: {e}\n----\n{s}\n----"))
}

pub fn parse_json_stderr(out: &std::process::Output) -> serde_json::Value {
    let s = std::str::from_utf8(&out.stderr).expect("stderr not utf-8");
    serde_json::from_str(s).unwrap_or_else(|e| panic!("stderr not JSON: {e}\n----\n{s}\n----"))
}
