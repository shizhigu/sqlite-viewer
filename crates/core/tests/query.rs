mod common;

use sqlv_core::{Db, OpenOpts, Page, Value};

#[test]
fn select_with_no_rows_returns_empty() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query("SELECT * FROM albums WHERE id = -1", &[], Page::default())
        .unwrap();
    assert!(res.rows.is_empty());
    assert!(!res.truncated);
    assert_eq!(res.columns.len(), 4);
}

#[test]
fn pagination_limit_truncates_flag_set() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id FROM albums ORDER BY id",
            &[],
            Page {
                limit: 2,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(res.rows.len(), 2);
    assert!(res.truncated);
}

#[test]
fn pagination_limit_exactly_matches_no_truncation() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id FROM albums ORDER BY id",
            &[],
            Page {
                limit: 4,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(res.rows.len(), 4);
    assert!(!res.truncated);
}

#[test]
fn pagination_offset_past_end_returns_empty() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id FROM albums ORDER BY id",
            &[],
            Page {
                limit: 10,
                offset: 999,
            },
        )
        .unwrap();
    assert!(res.rows.is_empty());
    assert!(!res.truncated);
}

#[test]
fn pagination_limit_zero_returns_empty_and_marks_truncated_when_rows_exist() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id FROM albums",
            &[],
            Page {
                limit: 0,
                offset: 0,
            },
        )
        .unwrap();
    assert!(res.rows.is_empty());
    assert!(res.truncated);
}

#[test]
fn parameter_binding_by_index() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT COUNT(*) FROM albums WHERE year >= ?1",
            &[Value::Integer(2000)],
            Page::default(),
        )
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Integer(1));
}

#[test]
fn parameter_binding_multiple_positional() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT COUNT(*) FROM albums WHERE year BETWEEN ?1 AND ?2",
            &[Value::Integer(1990), Value::Integer(1999)],
            Page::default(),
        )
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Integer(2));
}

#[test]
fn returns_null_values_as_value_null() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(x INTEGER, y TEXT);", &[]).unwrap();
    db.exec("INSERT INTO t(x,y) VALUES (NULL, NULL)", &[])
        .unwrap();
    let res = db
        .query("SELECT x, y FROM t", &[], Page::default())
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Null);
    assert_eq!(res.rows[0][1], Value::Null);
}

#[test]
fn empty_string_is_distinct_from_null() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(y TEXT);", &[]).unwrap();
    db.exec("INSERT INTO t(y) VALUES (NULL), ('')", &[])
        .unwrap();
    let res = db
        .query("SELECT y FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Null);
    assert_eq!(res.rows[1][0], Value::Text(String::new()));
}

#[test]
fn preserves_i64_extremes() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(n INTEGER);", &[]).unwrap();
    db.exec(
        "INSERT INTO t(n) VALUES (?1), (?2), (0), (-1);",
        &[Value::Integer(i64::MAX), Value::Integer(i64::MIN)],
    )
    .unwrap();
    let res = db
        .query("SELECT n FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Integer(i64::MAX));
    assert_eq!(res.rows[1][0], Value::Integer(i64::MIN));
    assert_eq!(res.rows[2][0], Value::Integer(0));
    assert_eq!(res.rows[3][0], Value::Integer(-1));
}

#[test]
fn roundtrips_real_values() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(r REAL);", &[]).unwrap();
    db.exec(
        "INSERT INTO t(r) VALUES (?1), (?2), (?3)",
        &[Value::Real(4.56789), Value::Real(-2.5e10), Value::Real(0.0)],
    )
    .unwrap();
    let res = db
        .query("SELECT r FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    let reals: Vec<f64> = res
        .rows
        .iter()
        .map(|r| match r[0] {
            Value::Real(f) => f,
            _ => panic!(),
        })
        .collect();
    assert!((reals[0] - 4.56789).abs() < 1e-9);
    assert!((reals[1] + 2.5e10).abs() < 1e-3);
    assert_eq!(reals[2], 0.0);
}

#[test]
fn roundtrips_blob_values() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(b BLOB);", &[]).unwrap();
    let payload = vec![0u8, 1, 2, 0xff, 0x10, 0x00];
    db.exec(
        "INSERT INTO t(b) VALUES (?1)",
        &[Value::Blob(payload.clone())],
    )
    .unwrap();
    let res = db.query("SELECT b FROM t", &[], Page::default()).unwrap();
    assert_eq!(res.rows[0][0], Value::Blob(payload));
}

#[test]
fn roundtrips_empty_blob() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(b BLOB);", &[]).unwrap();
    db.exec("INSERT INTO t(b) VALUES (?1)", &[Value::Blob(vec![])])
        .unwrap();
    let res = db.query("SELECT b FROM t", &[], Page::default()).unwrap();
    // SQLite collapses zero-length blob literals to NULL in some paths but
    // not via parameter binding — we expect a real empty blob here.
    assert_eq!(res.rows[0][0], Value::Blob(vec![]));
}

#[test]
fn unicode_text_preserved() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(s TEXT);", &[]).unwrap();
    db.exec(
        "INSERT INTO t(s) VALUES (?1), (?2), (?3)",
        &[
            Value::Text("Björk".into()),
            Value::Text("日本語".into()),
            Value::Text("🎵🎶".into()),
        ],
    )
    .unwrap();
    let res = db
        .query("SELECT s FROM t ORDER BY rowid", &[], Page::default())
        .unwrap();
    assert_eq!(res.rows[0][0], Value::Text("Björk".into()));
    assert_eq!(res.rows[1][0], Value::Text("日本語".into()));
    assert_eq!(res.rows[2][0], Value::Text("🎵🎶".into()));
}

#[test]
fn query_syntax_error_is_reported_as_sql_code() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let err = db
        .query("SELEKT * FROM albums", &[], Page::default())
        .unwrap_err();
    assert_eq!(err.code(), "sql");
}

#[test]
fn column_types_from_declared_schema() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id, title, year FROM albums LIMIT 1",
            &[],
            Page::default(),
        )
        .unwrap();
    assert_eq!(res.column_types[0].as_deref(), Some("INTEGER"));
    assert_eq!(res.column_types[1].as_deref(), Some("TEXT"));
    assert_eq!(res.column_types[2].as_deref(), Some("INTEGER"));
}

#[test]
fn column_types_none_for_computed_expressions() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query("SELECT COUNT(*), 1+1 FROM albums", &[], Page::default())
        .unwrap();
    assert_eq!(res.column_types[0], None);
    assert_eq!(res.column_types[1], None);
}

#[test]
fn exec_insert_update_delete_row_counts() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY, v INTEGER);", &[])
        .unwrap();

    let ins = db
        .exec("INSERT INTO t(v) VALUES (1),(2),(3);", &[])
        .unwrap();
    assert_eq!(ins.rows_affected, 3);
    assert!(ins.last_insert_rowid > 0);

    let upd = db.exec("UPDATE t SET v = v * 10;", &[]).unwrap();
    assert_eq!(upd.rows_affected, 3);

    let del = db.exec("DELETE FROM t WHERE v >= 20;", &[]).unwrap();
    assert_eq!(del.rows_affected, 2);
}

#[test]
fn exec_ddl_rows_affected_is_sticky_from_last_dml() {
    // Documents SQLite's sqlite3_changes() contract: it returns the count from
    // the most recent INSERT/UPDATE/DELETE, not zero after DDL. Agents reading
    // `rows_affected` should trust it only after DML statements.
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(v INTEGER);", &[]).unwrap();
    db.exec("INSERT INTO t(v) VALUES (1),(2),(3);", &[])
        .unwrap();
    let ddl = db.exec("CREATE INDEX ix_t_v ON t(v);", &[]).unwrap();
    assert_eq!(ddl.rows_affected, 3, "inherited from previous INSERT");
}

#[test]
fn exec_constraint_violation_is_sql_error() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    db.exec("CREATE TABLE t(x INTEGER NOT NULL);", &[]).unwrap();
    let err = db.exec("INSERT INTO t(x) VALUES (NULL);", &[]).unwrap_err();
    assert_eq!(err.code(), "sql");
}

#[test]
fn query_result_is_serde_serializable_and_stable_shape() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let res = db
        .query(
            "SELECT id, name FROM artists ORDER BY id LIMIT 1",
            &[],
            Page::default(),
        )
        .unwrap();
    let json = serde_json::to_value(&res).unwrap();
    assert!(json.get("columns").is_some());
    assert!(json.get("column_types").is_some());
    assert!(json.get("rows").is_some());
    assert!(json.get("truncated").is_some());
    assert!(json.get("elapsed_ms").is_some());
    assert_eq!(json["rows"][0][0], serde_json::json!(1));
    assert_eq!(json["rows"][0][1], serde_json::json!("Miles Davis"));
}
