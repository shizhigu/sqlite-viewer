//! Shared fixtures for core integration tests.
//!
//! `#[allow(dead_code)]` is needed because each integration-test binary
//! separately compiles this module and may use only a subset of helpers.

#![allow(dead_code)]

use sqlv_core::{Db, OpenOpts, Value};
use tempfile::NamedTempFile;

/// Build a musical-catalogue fixture DB: artists, albums, an index, a view,
/// a few rows. Returned `NamedTempFile` keeps the file alive — drop it and the
/// file disappears.
pub fn make_catalogue() -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    let db = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: Some(1_000),
        },
    )
    .unwrap();
    db.exec(
        "CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
        &[],
    )
    .unwrap();
    db.exec(
        "CREATE TABLE albums (\
           id INTEGER PRIMARY KEY,\
           title TEXT NOT NULL,\
           artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,\
           year INTEGER\
         );",
        &[],
    )
    .unwrap();
    db.exec("CREATE INDEX idx_albums_artist ON albums(artist_id);", &[])
        .unwrap();
    db.exec(
        "CREATE VIEW recent_albums AS SELECT * FROM albums WHERE year >= 2000;",
        &[],
    )
    .unwrap();
    db.exec(
        "INSERT INTO artists (name) VALUES (?1), (?2), (?3);",
        &[
            Value::Text("Miles Davis".into()),
            Value::Text("Björk".into()),
            Value::Text("Aphex Twin".into()),
        ],
    )
    .unwrap();
    db.exec(
        "INSERT INTO albums (title, artist_id, year) VALUES \
         ('Kind of Blue', 1, 1959), \
         ('Homogenic', 2, 1997), \
         ('Selected Ambient Works 85–92', 3, 1992), \
         ('Vespertine', 2, 2001);",
        &[],
    )
    .unwrap();
    file
}

/// Fresh empty DB — the connection itself is discarded, just keeps the file.
pub fn make_empty() -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    let _ = Db::open(
        file.path(),
        OpenOpts {
            read_only: false,
            timeout_ms: None,
        },
    )
    .unwrap();
    file
}
