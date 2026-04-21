use serde::Serialize;

/// App-level error delivered to the frontend. Stable `code` for UI branching,
/// free-form `message` for display.
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl From<sqlv_core::Error> for AppError {
    fn from(e: sqlv_core::Error) -> Self {
        AppError {
            code: e.code().to_string(),
            message: e.to_string(),
        }
    }
}

impl AppError {
    pub fn not_open() -> Self {
        AppError {
            code: "not_open".into(),
            message: "no database is currently open".into(),
        }
    }
}
