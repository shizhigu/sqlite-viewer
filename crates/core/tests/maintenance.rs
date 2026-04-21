mod common;

use sqlv_core::{Db, OpenOpts};

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
fn integrity_check_ok_on_clean_db() {
    let file = common::make_catalogue();
    let db = rw(&file);
    let r = db.integrity_check().unwrap();
    assert_eq!(r.task, "integrity_check");
    assert_eq!(r.output, vec!["ok".to_string()]);
}

#[test]
fn vacuum_succeeds() {
    let file = common::make_catalogue();
    let db = rw(&file);
    let r = db.vacuum().unwrap();
    assert_eq!(r.task, "vacuum");
    assert_eq!(r.output, vec!["done".to_string()]);
}

#[test]
fn analyze_and_reindex_no_target_succeed() {
    let file = common::make_catalogue();
    let db = rw(&file);
    assert_eq!(db.analyze(None).unwrap().task, "analyze");
    assert_eq!(db.reindex(None).unwrap().task, "reindex");
}

#[test]
fn analyze_scoped_to_table_succeeds() {
    let file = common::make_catalogue();
    let db = rw(&file);
    let r = db.analyze(Some("albums")).unwrap();
    assert_eq!(r.output, vec!["done".to_string()]);
}

#[test]
fn wal_checkpoint_invalid_mode_rejected() {
    let file = common::make_empty();
    let db = rw(&file);
    let err = db.wal_checkpoint("BANANAS").unwrap_err();
    assert_eq!(err.code(), "invalid");
}

#[test]
fn wal_checkpoint_truncate_produces_structured_output() {
    let file = common::make_empty();
    let db = rw(&file);
    // wal_checkpoint works even when not in WAL mode — it just reports
    // zero-frame rows. Skipping the mode switch avoids rusqlite's
    // ExecuteReturnedResults on the pragma result.
    let r = db.wal_checkpoint("TRUNCATE").unwrap();
    assert!(r.task.starts_with("wal_checkpoint"));
    assert_eq!(r.output.len(), 3);
    assert!(r.output[0].starts_with("busy="));
    assert!(r.output[1].starts_with("log="));
    assert!(r.output[2].starts_with("checkpointed="));
}

#[test]
fn maintenance_refuses_readonly_connection() {
    let file = common::make_catalogue();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    assert_eq!(db.vacuum().unwrap_err().code(), "readonly");
    assert_eq!(db.reindex(None).unwrap_err().code(), "readonly");
    assert_eq!(db.analyze(None).unwrap_err().code(), "readonly");
    assert_eq!(db.wal_checkpoint("PASSIVE").unwrap_err().code(), "readonly");
}
