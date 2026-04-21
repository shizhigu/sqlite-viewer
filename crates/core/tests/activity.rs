use sqlv_core::{ActivityEntry, ActivityLog, ActivityQuery};

fn temp_log() -> (tempfile::TempDir, ActivityLog) {
    let dir = tempfile::tempdir().unwrap();
    let log = ActivityLog::open_at(&dir.path().join("activity.db")).unwrap();
    (dir, log)
}

#[test]
fn open_creates_file_and_self_migrates() {
    let (_dir, log) = temp_log();
    // Can query an empty table immediately.
    let rows = log
        .query(&ActivityQuery {
            limit: 10,
            ..Default::default()
        })
        .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn append_and_recent() {
    let (_dir, log) = temp_log();
    let mut e = ActivityEntry::now("agent", "query");
    e.sql = Some("SELECT 1".into());
    e.db_path = Some("/tmp/x.sqlite".into());
    e.elapsed_ms = Some(12);
    e.rows = Some(1);
    let id = log.append(&e).unwrap();
    assert!(id > 0);

    let rows = log
        .query(&ActivityQuery {
            limit: 10,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source, "agent");
    assert_eq!(rows[0].sql.as_deref(), Some("SELECT 1"));
    assert_eq!(rows[0].elapsed_ms, Some(12));
}

#[test]
fn query_newest_first_ordering() {
    let (_dir, log) = temp_log();
    for i in 1..=5 {
        let mut e = ActivityEntry::now("ui", "query");
        e.ts_ms = 10_000 + i; // monotonic increasing
        e.sql = Some(format!("q{i}"));
        log.append(&e).unwrap();
    }
    let rows = log
        .query(&ActivityQuery {
            limit: 10,
            ..Default::default()
        })
        .unwrap();
    // Newest first: 10_005 → 10_001.
    assert_eq!(rows[0].ts_ms, 10_005);
    assert_eq!(rows[4].ts_ms, 10_001);
}

#[test]
fn grep_filter_is_case_insensitive_and_searches_sql_plus_path() {
    let (_dir, log) = temp_log();
    let mut a = ActivityEntry::now("agent", "query");
    a.sql = Some("SELECT * FROM Users".into());
    a.db_path = Some("/tmp/app.sqlite".into());
    log.append(&a).unwrap();

    let mut b = ActivityEntry::now("agent", "query");
    b.sql = Some("SELECT * FROM orders".into());
    b.db_path = Some("/tmp/checkout.sqlite".into());
    log.append(&b).unwrap();

    let filtered = log
        .query(&ActivityQuery {
            grep: Some("users".into()),
            limit: 10,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].sql.as_ref().unwrap().contains("Users"));
}

#[test]
fn filter_by_db_path_and_source() {
    let (_dir, log) = temp_log();
    for (src, path) in [
        ("ui", "/x.db"),
        ("ui", "/y.db"),
        ("agent", "/x.db"),
        ("agent", "/y.db"),
    ] {
        let mut e = ActivityEntry::now(src, "query");
        e.db_path = Some(path.into());
        log.append(&e).unwrap();
    }
    let only_agent_x = log
        .query(&ActivityQuery {
            source: Some("agent".into()),
            db_path: Some("/x.db".into()),
            limit: 10,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(only_agent_x.len(), 1);
    assert_eq!(only_agent_x[0].source, "agent");
    assert_eq!(only_agent_x[0].db_path.as_deref(), Some("/x.db"));
}

#[test]
fn prune_before_deletes_older_rows() {
    let (_dir, log) = temp_log();
    for ts in [100, 200, 300, 400, 500] {
        let mut e = ActivityEntry::now("ui", "query");
        e.ts_ms = ts;
        log.append(&e).unwrap();
    }
    let deleted = log.prune_before(300).unwrap();
    assert_eq!(deleted, 2);
    let left: Vec<i64> = log
        .query(&ActivityQuery {
            limit: 10,
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .map(|r| r.ts_ms)
        .collect();
    assert_eq!(left, vec![500, 400, 300]);
}
