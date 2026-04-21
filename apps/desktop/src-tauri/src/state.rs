use std::sync::Mutex;

use sqlv_core::Db;

/// Holds the single currently-open `Db`. The desktop app opens one DB at a
/// time (v1) — multi-connection support is out of scope for MVP.
///
/// This is shared between Tauri commands (via `State<Arc<AppState>>`) and
/// the push-server thread (which gets the same `Arc<AppState>` directly),
/// so the HTTP server and the UI agree on which DB is open.
#[derive(Default)]
pub struct AppState {
    pub current: Mutex<Option<Db>>,
}
