pub const EXIT_OK: i32 = 0;
pub const EXIT_OTHER: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_NOT_FOUND: i32 = 3;
pub const EXIT_READONLY: i32 = 4;
pub const EXIT_SQL: i32 = 5;

#[allow(dead_code)]
pub const EXIT_USAGE_ALIAS: i32 = EXIT_USAGE;

/// What a failing command emits — a stable `code` string, a human message,
/// and a matching process-exit code.
#[derive(Debug)]
pub struct Failure {
    code: &'static str,
    message: String,
    exit: i32,
}

impl Failure {
    pub fn new(code: &'static str, message: impl Into<String>, exit: i32) -> Self {
        Self { code, message: message.into(), exit }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new("usage", message.into(), EXIT_USAGE)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> String {
        self.message.clone()
    }

    pub fn exit_code(&self) -> i32 {
        self.exit
    }
}

impl From<sqlv_core::Error> for Failure {
    fn from(e: sqlv_core::Error) -> Self {
        let (code, exit) = match e.code() {
            "not_found" => ("not_found", EXIT_NOT_FOUND),
            "readonly" => ("readonly", EXIT_READONLY),
            "sql" => ("sql", EXIT_SQL),
            "invalid" => ("invalid", EXIT_OTHER),
            "io" => ("io", EXIT_OTHER),
            other => {
                // Keep the string alive for the static lifetime requirement.
                // New core error codes should be added above; unknown ones fall
                // through as generic "other".
                let _ = other;
                ("other", EXIT_OTHER)
            }
        };
        Self { code, message: e.to_string(), exit }
    }
}
