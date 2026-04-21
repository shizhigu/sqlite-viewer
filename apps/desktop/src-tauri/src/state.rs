use std::sync::Mutex;

use sqlv_core::{ActivityLog, CancelHandle, Db};

/// Holds the single currently-open `Db`. The desktop app opens one DB at a
/// time (v1) — multi-connection support is out of scope for MVP.
///
/// This is shared between Tauri commands (via `State<Arc<AppState>>`) and
/// the push-server thread (which gets the same `Arc<AppState>` directly),
/// so the HTTP server and the UI agree on which DB is open.
#[derive(Default)]
pub struct AppState {
    pub current: Mutex<Option<Db>>,
    /// Cancel handle for the currently-running query, if any. The handle
    /// itself is cheap (Arc<InterruptHandle>) so we stash it in the state
    /// for cross-thread access from the "Cancel" button.
    pub cancel: Mutex<Option<CancelHandle>>,
    /// Persistent SQLite-backed activity log, opened lazily. `None` means
    /// we failed to open it and should degrade silently.
    pub activity: Mutex<Option<ActivityLog>>,
}
