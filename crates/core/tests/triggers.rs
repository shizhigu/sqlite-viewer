mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn empty_db_has_no_triggers() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    assert!(db.triggers().unwrap().is_empty());
}

#[test]
fn trigger_listed_with_table_and_sql() {
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
        "CREATE TABLE t(id INTEGER PRIMARY KEY, n INTEGER, updated_at TEXT);",
        &[],
    )
    .unwrap();
    db.exec(
        "CREATE TRIGGER trg_touch AFTER UPDATE ON t FOR EACH ROW \
         BEGIN UPDATE t SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;",
        &[],
    )
    .unwrap();

    let trs = db.triggers().unwrap();
    assert_eq!(trs.len(), 1);
    assert_eq!(trs[0].name, "trg_touch");
    assert_eq!(trs[0].table, "t");
    assert!(trs[0].sql.as_deref().unwrap_or("").contains("AFTER UPDATE"));
}

#[test]
fn triggers_sorted_by_table_then_name() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE a(x INT);", &[]).unwrap();
    db.exec("CREATE TABLE b(x INT);", &[]).unwrap();
    db.exec(
        "CREATE TRIGGER t_b_z BEFORE INSERT ON b BEGIN SELECT 1; END;",
        &[],
    )
    .unwrap();
    db.exec(
        "CREATE TRIGGER t_a_m BEFORE INSERT ON a BEGIN SELECT 1; END;",
        &[],
    )
    .unwrap();
    db.exec(
        "CREATE TRIGGER t_a_a BEFORE INSERT ON a BEGIN SELECT 1; END;",
        &[],
    )
    .unwrap();

    let names: Vec<_> = db.triggers().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(names, vec!["t_a_a", "t_a_m", "t_b_z"]);
}
