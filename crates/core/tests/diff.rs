mod common;

use sqlv_core::{diff_schemas, Db, OpenOpts};

fn rw(f: &tempfile::NamedTempFile) -> Db {
    Db::open(
        f.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap()
}

#[test]
fn identical_schemas_report_no_changes() {
    let f1 = common::make_catalogue();
    let f2 = common::make_catalogue();
    let a = rw(&f1);
    let b = rw(&f2);
    let r = diff_schemas(&a, &b).unwrap();
    assert!(r.only_in_a.is_empty());
    assert!(r.only_in_b.is_empty());
    assert!(r.changed.is_empty());
}

#[test]
fn added_and_removed_tables_are_surfaced() {
    let f1 = common::make_empty();
    let f2 = common::make_empty();
    let a = rw(&f1);
    let b = rw(&f2);
    a.exec("CREATE TABLE only_in_a(x INT);", &[]).unwrap();
    a.exec("CREATE TABLE shared(x INT);", &[]).unwrap();
    b.exec("CREATE TABLE shared(x INT);", &[]).unwrap();
    b.exec("CREATE TABLE only_in_b(x INT);", &[]).unwrap();

    let r = diff_schemas(&a, &b).unwrap();
    assert_eq!(r.only_in_a, vec!["only_in_a".to_string()]);
    assert_eq!(r.only_in_b, vec!["only_in_b".to_string()]);
    assert!(r.changed.is_empty());
}

#[test]
fn column_add_remove_and_type_change_detected() {
    let f1 = common::make_empty();
    let f2 = common::make_empty();
    let a = rw(&f1);
    let b = rw(&f2);
    a.exec("CREATE TABLE u(id INTEGER, name TEXT, old_col TEXT);", &[])
        .unwrap();
    b.exec(
        "CREATE TABLE u(id INTEGER, name VARCHAR, new_col REAL);",
        &[],
    )
    .unwrap();

    let r = diff_schemas(&a, &b).unwrap();
    assert_eq!(r.changed.len(), 1);
    let u = &r.changed[0];
    assert_eq!(u.name, "u");
    assert_eq!(u.columns_added[0].name, "new_col");
    assert_eq!(u.columns_removed[0].name, "old_col");
    let changed_names: Vec<&str> = u.columns_changed.iter().map(|c| c.name.as_str()).collect();
    assert!(changed_names.contains(&"name"), "got {changed_names:?}");
    let reason = u.columns_changed[0].reasons.join(" ");
    assert!(reason.contains("type:"), "reasons: {reason}");
}

#[test]
fn indexes_added_removed() {
    let f1 = common::make_empty();
    let f2 = common::make_empty();
    let a = rw(&f1);
    let b = rw(&f2);
    a.exec("CREATE TABLE t(x INT, y INT);", &[]).unwrap();
    b.exec("CREATE TABLE t(x INT, y INT);", &[]).unwrap();
    a.exec("CREATE INDEX idx_t_x ON t(x);", &[]).unwrap();
    b.exec("CREATE INDEX idx_t_y ON t(y);", &[]).unwrap();

    let r = diff_schemas(&a, &b).unwrap();
    assert_eq!(r.changed.len(), 1);
    let t = &r.changed[0];
    assert_eq!(t.indexes_removed[0].name, "idx_t_x");
    assert_eq!(t.indexes_added[0].name, "idx_t_y");
}

#[test]
fn pk_and_not_null_changes_detected() {
    let f1 = common::make_empty();
    let f2 = common::make_empty();
    let a = rw(&f1);
    let b = rw(&f2);
    a.exec("CREATE TABLE t(x INTEGER, y TEXT);", &[]).unwrap();
    b.exec(
        "CREATE TABLE t(x INTEGER PRIMARY KEY, y TEXT NOT NULL);",
        &[],
    )
    .unwrap();
    let r = diff_schemas(&a, &b).unwrap();
    let t = &r.changed[0];
    let reasons: String = t
        .columns_changed
        .iter()
        .flat_map(|c| c.reasons.iter().cloned())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(reasons.contains("pk"), "reasons: {reasons}");
    assert!(reasons.contains("not_null"), "reasons: {reasons}");
}
