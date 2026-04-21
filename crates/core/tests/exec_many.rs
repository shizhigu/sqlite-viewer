mod common;

use sqlv_core::{Db, OpenOpts, Page, Value};

#[test]
fn all_statements_commit_when_every_one_succeeds() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec("CREATE TABLE t(v INTEGER);", &[]).unwrap();

    let stmts: &[(&str, &[Value])] = &[
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(1)]),
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(2)]),
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(3)]),
    ];
    let res = db.exec_many(stmts).unwrap();
    assert_eq!(res.rows_affected, 3);
    assert!(res.last_insert_rowid > 0);

    let q = db.query("SELECT COUNT(*) FROM t", &[], Page::default()).unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(3));
}

#[test]
fn whole_batch_rolls_back_on_failure() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec("CREATE TABLE t(v INTEGER NOT NULL);", &[]).unwrap();

    let stmts: &[(&str, &[Value])] = &[
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(1)]),
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(2)]),
        // This one will fail — NOT NULL violation.
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Null]),
        ("INSERT INTO t(v) VALUES (?1)", &[Value::Integer(4)]),
    ];
    let err = db.exec_many(stmts).unwrap_err();
    assert_eq!(err.code(), "sql");

    // Nothing should have committed.
    let q = db.query("SELECT COUNT(*) FROM t", &[], Page::default()).unwrap();
    assert_eq!(q.rows[0][0], Value::Integer(0));
}

#[test]
fn readonly_rejects_exec_many() {
    let fixture = common::make_catalogue();
    let db = Db::open(fixture.path(), OpenOpts::default()).unwrap();
    let stmts: &[(&str, &[Value])] = &[("INSERT INTO artists(name) VALUES ('x')", &[])];
    let err = db.exec_many(stmts).unwrap_err();
    assert_eq!(err.code(), "readonly");
}

#[test]
fn empty_batch_is_a_noop_that_still_commits() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    let res = db.exec_many(&[]).unwrap();
    assert_eq!(res.rows_affected, 0);
    assert_eq!(res.last_insert_rowid, 0);
}

#[test]
fn mixed_dml_updates_row_counts_and_rolls_back_on_constraint() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts { read_only: false, timeout_ms: None }).unwrap();
    db.exec(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, balance INTEGER CHECK (balance >= 0));",
        &[],
    )
    .unwrap();
    db.exec("INSERT INTO accounts(id,balance) VALUES (1, 100),(2, 50);", &[]).unwrap();

    // Transfer 60 from 1 → 2, which would leave account 1 at 40 and 2 at 110.
    // Then a bogus statement drains account 1 past zero — CHECK fails.
    let stmts: &[(&str, &[Value])] = &[
        (
            "UPDATE accounts SET balance = balance - ?1 WHERE id = 1",
            &[Value::Integer(60)],
        ),
        (
            "UPDATE accounts SET balance = balance + ?1 WHERE id = 2",
            &[Value::Integer(60)],
        ),
        (
            "UPDATE accounts SET balance = balance - ?1 WHERE id = 1",
            &[Value::Integer(999)],
        ),
    ];
    let err = db.exec_many(stmts).unwrap_err();
    assert_eq!(err.code(), "sql");

    // Balances should be restored to their original state.
    let res = db
        .query("SELECT id, balance FROM accounts ORDER BY id", &[], Page::default())
        .unwrap();
    assert_eq!(res.rows[0], vec![Value::Integer(1), Value::Integer(100)]);
    assert_eq!(res.rows[1], vec![Value::Integer(2), Value::Integer(50)]);
}
