mod common;

use sqlv_core::{Db, OpenOpts};
use std::path::PathBuf;

#[test]
fn default_opts_are_readonly_with_timeout() {
    let opts = OpenOpts::default();
    assert!(opts.read_only);
    assert_eq!(opts.timeout_ms, Some(5_000));
}

#[test]
fn open_readonly_on_existing_file_succeeds() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    assert!(db.is_read_only());
    assert_eq!(db.path(), fixture.path());
}

#[test]
fn open_readonly_on_missing_file_fails() {
    let missing = PathBuf::from("/tmp/sqlv-nonexistent-db-for-test.sqlite");
    let _ = std::fs::remove_file(&missing);
    let err = Db::open(&missing, OpenOpts::default()).unwrap_err();
    assert_eq!(err.code(), "sql");
}

#[test]
fn open_readwrite_on_missing_file_creates_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.db");
    assert!(!path.exists());
    let db = Db::open(
        &path,
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    assert!(!db.is_read_only());
    drop(db);
    assert!(path.exists());
}

#[test]
fn writes_rejected_when_readonly() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db
        .exec("INSERT INTO artists(name) VALUES ('nope')", &[])
        .unwrap_err();
    assert_eq!(err.code(), "readonly");
}

#[test]
fn writes_allowed_when_readwrite() {
    let fixture = common::make_catalogue();
    let db = Db::open(
        fixture.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let res = db
        .exec("INSERT INTO artists(name) VALUES ('Test')", &[])
        .unwrap();
    assert_eq!(res.rows_affected, 1);
    assert!(res.last_insert_rowid > 0);
}

#[test]
fn foreign_keys_are_enabled_by_default() {
    // Reopen read-write and try to violate FK; should error, proving FKs are on.
    let fixture = common::make_catalogue();
    let db = Db::open(
        fixture.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let err = db
        .exec(
            "INSERT INTO albums (title, artist_id, year) VALUES ('Bad', 999, 2020)",
            &[],
        )
        .unwrap_err();
    assert_eq!(err.code(), "sql");
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("foreign key") || msg.to_lowercase().contains("constraint"),
        "expected FK/constraint error, got {msg}"
    );
}

#[test]
fn two_concurrent_readonly_connections() {
    let fixture = common::make_catalogue();
    let a = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let b = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    assert_eq!(a.tables().unwrap().len(), 2);
    assert_eq!(b.tables().unwrap().len(), 2);
}
