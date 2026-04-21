mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn schema_surfaces_virtual_generated_column() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec(
        "CREATE TABLE t(a INTEGER, b INTEGER AS (a * 2) VIRTUAL, c INTEGER AS (a + 1) STORED);",
        &[],
    )
    .unwrap();

    let s = db.schema("t").unwrap();
    let a = s.columns.iter().find(|c| c.name == "a").unwrap();
    let b = s.columns.iter().find(|c| c.name == "b").unwrap();
    let c = s.columns.iter().find(|c| c.name == "c").unwrap();

    assert_eq!(a.hidden, 0, "normal column");
    assert_eq!(b.hidden, 2, "VIRTUAL generated column");
    assert_eq!(c.hidden, 3, "STORED generated column");
}

#[test]
fn schema_marks_fts5_shadow_columns_hidden() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    // FTS5 is bundled in rusqlite's sqlite build.
    db.exec("CREATE VIRTUAL TABLE docs USING fts5(body);", &[])
        .unwrap();
    let s = db.schema("docs").unwrap();
    // FTS5 exposes the primary column plus several hidden ones (rank, etc.).
    // We don't hard-code which — just assert that at least one column is
    // flagged hidden so the frontend can gate edits.
    let hidden_count = s.columns.iter().filter(|c| c.hidden != 0).count();
    assert!(
        hidden_count >= 1,
        "expected at least one hidden column on FTS5 virtual table, got {:?}",
        s.columns
    );
}

#[test]
fn schema_ordinary_table_has_all_hidden_zero() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let s = db.schema("albums").unwrap();
    assert!(s.columns.iter().all(|c| c.hidden == 0));
}
