mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn read_simple_pragma() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let p = db.pragma("user_version", None).unwrap();
    assert_eq!(p.name, "user_version");
    assert_eq!(p.values, vec![vec!["0".to_string()]]);
}

#[test]
fn read_encoding_pragma_returns_text() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let p = db.pragma("encoding", None).unwrap();
    assert_eq!(p.values, vec![vec!["UTF-8".to_string()]]);
}

#[test]
fn set_pragma_requires_readwrite() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db.pragma("user_version", Some("5")).unwrap_err();
    assert_eq!(err.code(), "readonly");
}

#[test]
fn set_pragma_numeric_value() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let p = db.pragma("user_version", Some("42")).unwrap();
    assert_eq!(p.values, vec![vec!["42".to_string()]]);
}

#[test]
fn set_pragma_keyword_value() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    // journal_mode accepts bare keywords like WAL, DELETE, MEMORY.
    let p = db.pragma("journal_mode", Some("MEMORY")).unwrap();
    assert_eq!(p.values[0][0].to_lowercase(), "memory");
}

#[test]
fn rejects_pragma_name_with_injection_chars() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db
        .pragma("user_version; DROP TABLE artists", None)
        .unwrap_err();
    assert_eq!(err.code(), "invalid");
}

#[test]
fn rejects_pragma_value_with_injection_chars() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let err = db
        .pragma("user_version", Some("1; DROP TABLE t; --"))
        .unwrap_err();
    assert_eq!(err.code(), "invalid");
}

#[test]
fn multi_row_pragma_returns_all_rows() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    // `database_list` returns one row per attached database.
    let p = db.pragma("database_list", None).unwrap();
    assert!(!p.values.is_empty());
    // Shape: [seq, name, file]
    assert!(p.values.iter().all(|r| r.len() == 3));
    assert!(p.values.iter().any(|r| r[1] == "main"));
}

#[test]
fn unknown_pragma_returns_empty() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    // Unknown pragmas don't error in SQLite — they just yield no rows.
    let p = db.pragma("not_a_real_pragma_abcxyz", None).unwrap();
    assert!(p.values.is_empty());
}
