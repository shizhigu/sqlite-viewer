mod common;

use sqlv_core::{Db, OpenOpts, TableKind};

#[test]
fn empty_db_has_no_tables_no_views_no_indexes() {
    let fixture = common::make_empty();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    assert!(db.tables().unwrap().is_empty());
    assert!(db.views().unwrap().is_empty());
    assert!(db.indexes(None).unwrap().is_empty());
}

#[test]
fn tables_excludes_sqlite_internal() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let names: Vec<String> = db.tables().unwrap().into_iter().map(|t| t.name).collect();
    assert!(!names.iter().any(|n| n.starts_with("sqlite_")));
    assert_eq!(names, vec!["albums", "artists"]);
}

#[test]
fn tables_returns_row_counts_including_zero() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec("CREATE TABLE t1(x INTEGER);", &[]).unwrap();
    db.exec("CREATE TABLE t2(x INTEGER);", &[]).unwrap();
    db.exec("INSERT INTO t2 VALUES (1),(2),(3);", &[]).unwrap();

    let tables = db.tables().unwrap();
    let t1 = tables.iter().find(|t| t.name == "t1").unwrap();
    let t2 = tables.iter().find(|t| t.name == "t2").unwrap();
    assert_eq!(t1.row_count, Some(0));
    assert_eq!(t2.row_count, Some(3));
}

#[test]
fn views_returned_with_sql() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let views = db.views().unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "recent_albums");
    assert!(views[0].sql.as_deref().unwrap_or("").contains("year >= 2000"));
}

#[test]
fn indexes_filter_by_table() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();

    let idx_albums = db.indexes(Some("albums")).unwrap();
    assert!(idx_albums.iter().any(|i| i.name == "idx_albums_artist"));
    assert!(idx_albums.iter().all(|i| i.table == "albums"));

    let idx_artists = db.indexes(Some("artists")).unwrap();
    assert!(idx_artists.iter().all(|i| i.table == "artists"));
}

#[test]
fn indexes_missing_table_is_not_found() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db.indexes(Some("no_such_table")).unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[test]
fn indexes_reports_unique_and_origin() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec(
        "CREATE TABLE t (a INTEGER, b TEXT UNIQUE);",
        &[],
    )
    .unwrap();
    db.exec("CREATE INDEX ix_t_a ON t(a);", &[]).unwrap();

    let idx = db.indexes(Some("t")).unwrap();
    let manual = idx.iter().find(|i| i.name == "ix_t_a").unwrap();
    assert!(!manual.unique);
    assert_eq!(manual.origin, "c");

    let uniq = idx.iter().find(|i| i.origin == "u").unwrap();
    assert!(uniq.unique);
    assert_eq!(uniq.columns, vec!["b"]);
}

#[test]
fn schema_describes_columns_pk_fk_and_indexes() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();

    let schema = db.schema("albums").unwrap();
    assert!(matches!(schema.kind, TableKind::Table));
    assert_eq!(schema.columns.len(), 4);

    let pk: Vec<_> = schema.columns.iter().filter(|c| c.pk > 0).map(|c| c.name.as_str()).collect();
    assert_eq!(pk, vec!["id"]);

    let title = schema.columns.iter().find(|c| c.name == "title").unwrap();
    assert!(title.not_null);
    assert_eq!(title.decl_type.as_deref(), Some("TEXT"));

    assert_eq!(schema.foreign_keys.len(), 1);
    assert_eq!(schema.foreign_keys[0].on_delete, "CASCADE");

    assert!(schema.indexes.iter().any(|i| i.name == "idx_albums_artist"));
}

#[test]
fn schema_handles_composite_primary_key() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec(
        "CREATE TABLE compo (\
           a INTEGER NOT NULL, \
           b TEXT NOT NULL, \
           c REAL, \
           PRIMARY KEY (a, b)\
         );",
        &[],
    )
    .unwrap();

    let schema = db.schema("compo").unwrap();
    let mut pk_cols: Vec<(i32, &str)> = schema
        .columns
        .iter()
        .filter(|c| c.pk > 0)
        .map(|c| (c.pk, c.name.as_str()))
        .collect();
    pk_cols.sort_by_key(|(p, _)| *p);
    assert_eq!(pk_cols, vec![(1, "a"), (2, "b")]);
}

#[test]
fn schema_default_values_are_surfaced() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec(
        "CREATE TABLE d (\
           a INTEGER DEFAULT 7, \
           b TEXT DEFAULT 'hi', \
           c INTEGER\
         );",
        &[],
    )
    .unwrap();
    let s = db.schema("d").unwrap();
    let a = s.columns.iter().find(|c| c.name == "a").unwrap();
    let b = s.columns.iter().find(|c| c.name == "b").unwrap();
    let c = s.columns.iter().find(|c| c.name == "c").unwrap();
    assert_eq!(a.default_value.as_deref(), Some("7"));
    assert_eq!(b.default_value.as_deref(), Some("'hi'"));
    assert_eq!(c.default_value, None);
}

#[test]
fn schema_view_has_no_foreign_keys_or_indexes() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let s = db.schema("recent_albums").unwrap();
    assert!(matches!(s.kind, TableKind::View));
    assert!(s.foreign_keys.is_empty());
    assert!(s.indexes.is_empty());
    // Views still expose their projected column list via PRAGMA table_info.
    assert!(!s.columns.is_empty());
}

#[test]
fn schema_missing_object_is_not_found() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db.schema("no_such").unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[test]
fn schema_handles_identifier_with_spaces_and_quotes() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    // Table name with a space and an embedded double-quote. quote_ident must
    // escape the inner quote; otherwise the CREATE would fail or the later
    // PRAGMA would reference the wrong object.
    db.exec(
        "CREATE TABLE \"weird \"\"name\" (id INTEGER PRIMARY KEY, note TEXT);",
        &[],
    )
    .unwrap();
    db.exec(
        "INSERT INTO \"weird \"\"name\"(note) VALUES ('hello');",
        &[],
    )
    .unwrap();

    let tables = db.tables().unwrap();
    let t = tables.iter().find(|t| t.name == "weird \"name").unwrap();
    assert_eq!(t.row_count, Some(1));

    let schema = db.schema("weird \"name").unwrap();
    assert_eq!(schema.columns.len(), 2);
}

#[test]
fn tables_are_sorted_case_sensitive_ascending() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec("CREATE TABLE zebra(x INT);", &[]).unwrap();
    db.exec("CREATE TABLE apple(x INT);", &[]).unwrap();
    db.exec("CREATE TABLE mango(x INT);", &[]).unwrap();
    let names: Vec<String> = db.tables().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(names, vec!["apple", "mango", "zebra"]);
}

#[test]
fn schema_without_rowid_still_has_pk_columns() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec(
        "CREATE TABLE w (a INTEGER NOT NULL, b TEXT NOT NULL, PRIMARY KEY(a,b)) WITHOUT ROWID;",
        &[],
    )
    .unwrap();
    let s = db.schema("w").unwrap();
    assert_eq!(s.columns.len(), 2);
    assert!(s.columns.iter().all(|c| c.pk > 0));
}
