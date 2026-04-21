mod common;

use std::thread;
use std::time::Duration;

use sqlv_core::{Db, OpenOpts, Page};

/// Spin up a long-running recursive CTE, cancel it from another thread,
/// assert the worker returns a SQLite interrupt error quickly.
#[test]
fn cancel_handle_interrupts_running_query() {
    let file = common::make_empty();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    let cancel = db.cancel_handle();

    // Fire the cancel from a second thread after a short wait — the query
    // below would otherwise run for minutes.
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(80));
        cancel.cancel();
    });

    let start = std::time::Instant::now();
    // Recursive CTE that counts to a billion — never terminates naturally
    // in a reasonable time window, so cancel is the only way out.
    let sql = "WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r) \
               SELECT COUNT(*) FROM r LIMIT 1";
    let res = db.query(sql, &[], Page::default());
    canceller.join().unwrap();

    assert!(
        res.is_err(),
        "expected cancel to surface as an error, got Ok"
    );
    let err = res.unwrap_err();
    // rusqlite maps SQLITE_INTERRUPT into the sql code.
    assert_eq!(err.code(), "sql");
    // Guard against the test accidentally waiting the full billion rows.
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "cancel didn't fire fast enough ({:?})",
        start.elapsed()
    );
}

#[test]
fn cancel_on_idle_connection_is_a_noop() {
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    // No query in flight — cancel() must not panic, leave the connection
    // in a usable state.
    db.cancel_handle().cancel();
    // Verify connection still answers.
    let res = db.query("SELECT 1", &[], Page::default()).unwrap();
    assert_eq!(res.rows.len(), 1);
}

#[test]
fn cancel_handle_is_clone_and_send() {
    // Regression: CancelHandle must cross thread boundaries so the HTTP
    // server can park it in AppState while a different thread runs the
    // query.
    let file = common::make_empty();
    let db = Db::open(file.path(), OpenOpts::default()).unwrap();
    let h1 = db.cancel_handle();
    let h2 = h1.clone();
    thread::spawn(move || h2.cancel()).join().unwrap();
    // h1 should still be usable afterwards.
    h1.cancel();
}
