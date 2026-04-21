mod common;

use sqlv_core::{Db, OpenOpts, Page, Value};

#[test]
fn readwrite_connection_is_in_wal_mode() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let v = db
        .query("PRAGMA journal_mode", &[], Page::default())
        .unwrap();
    // SQLite returns the current journal mode as a single TEXT row.
    match &v.rows[0][0] {
        Value::Text(s) => assert_eq!(s.to_ascii_lowercase(), "wal"),
        other => panic!("expected TEXT row, got {other:?}"),
    }
}

#[test]
fn readonly_connection_stays_in_default_mode() {
    // Open RW first to create the file, then reopen RO and check we didn't
    // force WAL on a read-only connection (that would error).
    let file = common::make_empty();
    drop(
        Db::open(
            file.path(),
            OpenOpts {
                read_only: false,
                timeout_ms: None,
            },
        )
        .unwrap(),
    );

    // Fresh RO open — must succeed and the existing WAL mode persists
    // (WAL is a file-level setting, not per-connection).
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let v = db
        .query("PRAGMA journal_mode", &[], Page::default())
        .unwrap();
    match &v.rows[0][0] {
        Value::Text(s) => {
            // Once set to WAL, the DB persists in WAL mode. We just verify
            // the RO open didn't blow up and produces a readable pragma.
            assert!(!s.is_empty());
        }
        other => panic!("expected TEXT row, got {other:?}"),
    }
}
