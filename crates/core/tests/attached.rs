mod common;

use sqlv_core::{Db, OpenOpts};

#[test]
fn schemas_reports_main_by_default() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let schemas = db.schemas().unwrap();
    assert!(
        schemas.iter().any(|s| s.name == "main"),
        "expected `main` in {schemas:?}",
    );
    // `main` always has seq 0.
    assert_eq!(schemas.iter().find(|s| s.name == "main").unwrap().seq, 0);
}

#[test]
fn schemas_surfaces_attached_databases() {
    let primary = common::make_catalogue();
    let secondary = common::make_catalogue();
    let db = Db::open(
        primary.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    // `ATTACH DATABASE 'path' AS alias` — path must be a single-quoted literal.
    let sql = format!(
        "ATTACH DATABASE '{}' AS extra",
        secondary.path().to_string_lossy().replace('\'', "''"),
    );
    db.exec(&sql, &[]).unwrap();

    let schemas = db.schemas().unwrap();
    let names: Vec<&str> = schemas.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"main"), "got {names:?}");
    assert!(names.contains(&"extra"), "got {names:?}");
    let extra = schemas.iter().find(|s| s.name == "extra").unwrap();
    assert!(extra.seq >= 2);
    assert!(!extra.file.is_empty(), "attached file path should be set");
}

#[test]
fn tables_in_schema_main_matches_default_tables() {
    let file = common::make_catalogue();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let default_names: Vec<String> = db.tables().unwrap().into_iter().map(|t| t.name).collect();
    let main_names: Vec<String> = db
        .tables_in_schema("main")
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(default_names, main_names);
}

#[test]
fn tables_in_schema_attached_returns_attached_tables() {
    let primary = common::make_catalogue(); // has `artists`, `albums`
    let secondary = common::make_empty();
    let db = Db::open(
        primary.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let sql = format!(
        "ATTACH DATABASE '{}' AS extra",
        secondary.path().to_string_lossy().replace('\'', "''"),
    );
    db.exec(&sql, &[]).unwrap();
    db.exec("CREATE TABLE extra.only_here(x INT);", &[])
        .unwrap();

    let main = db.tables_in_schema("main").unwrap();
    let extra = db.tables_in_schema("extra").unwrap();
    assert!(main.iter().any(|t| t.name == "artists"));
    assert!(extra.iter().any(|t| t.name == "only_here"));
    assert!(!main.iter().any(|t| t.name == "only_here"));
}
