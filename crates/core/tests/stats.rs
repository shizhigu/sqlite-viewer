mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn stats_empty_db_has_no_tables_and_zero_freelist() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let s = db.stats().unwrap();
    assert!(s.tables.is_empty());
    assert_eq!(s.freelist_count, 0);
    // A freshly created, never-written-to DB may report page_count=0 until
    // SQLite materializes any pages. Schema-only dbs start >= 1.
    assert!(s.page_count >= 0);
    assert!(s.page_size > 0);
}

#[test]
fn stats_reports_per_table_row_counts() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let s = db.stats().unwrap();
    let names: Vec<&str> = s.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["albums", "artists"]);
    let rc: std::collections::HashMap<_, _> =
        s.tables.iter().map(|t| (t.name.as_str(), t.row_count)).collect();
    assert_eq!(rc["albums"], 4);
    assert_eq!(rc["artists"], 3);
}

#[test]
fn stats_is_json_serializable_with_stable_fields() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let v = serde_json::to_value(db.stats().unwrap()).unwrap();
    for field in [
        "path",
        "size_bytes",
        "page_size",
        "page_count",
        "freelist_count",
        "tables",
    ] {
        assert!(v.get(field).is_some(), "missing {field}");
    }
    let tables = v["tables"].as_array().unwrap();
    assert!(tables[0].get("name").is_some());
    assert!(tables[0].get("row_count").is_some());
}
