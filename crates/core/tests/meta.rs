mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn meta_reports_file_and_engine_info() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let m = db.meta().unwrap();

    assert_eq!(m.path, fixture.path().display().to_string());
    assert!(m.size_bytes > 0);
    assert!(m.page_size >= 512 && (m.page_size & (m.page_size - 1)) == 0); // power of 2
    assert!(m.page_count > 0);
    assert_eq!(m.encoding, "UTF-8");
    assert_eq!(m.user_version, 0);
    assert_eq!(m.application_id, 0);
    assert!(!m.sqlite_library_version.is_empty());
    assert!(m.read_only);
}

#[test]
fn meta_is_json_serializable_with_stable_fields() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let json = serde_json::to_value(db.meta().unwrap()).unwrap();
    for field in [
        "path",
        "size_bytes",
        "page_size",
        "page_count",
        "encoding",
        "user_version",
        "application_id",
        "journal_mode",
        "sqlite_library_version",
        "read_only",
    ] {
        assert!(json.get(field).is_some(), "missing field: {field}");
    }
}

#[test]
fn meta_reflects_user_version_pragma() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec("PRAGMA user_version = 42", &[]).unwrap();
    let m = db.meta().unwrap();
    assert_eq!(m.user_version, 42);
}

#[test]
fn meta_reflects_readwrite_flag() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    assert!(!db.meta().unwrap().read_only);
}
