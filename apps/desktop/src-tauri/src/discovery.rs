//! Instance discovery + authentication token.
//!
//! Every running desktop instance owns one file under
//! `~/.sqlv/instances/<pid>.json`:
//!
//! ```json
//! { "pid": 12345, "port": 50500, "token": "…hex…", "started_at": "2026-04-21T…" }
//! ```
//!
//! The CLI (`sqlv push`) enumerates this directory to find live instances
//! instead of brute-scanning port 50500..=50509. Each file is deleted
//! when `Instance` is dropped at app shutdown.
//!
//! The `token` is a 24-byte random string (48 hex chars) required in the
//! `X-Sqlv-Token` HTTP header on every request except `/health`. Prevents
//! casual local-process DB snooping.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstanceInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub started_at: String,
}

/// Drop-guard: removes the instance file on app shutdown.
pub struct Instance {
    path: PathBuf,
    pub info: InstanceInfo,
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn dir() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))?;
    let dir = home.join(".sqlv").join("instances");
    fs::create_dir_all(&dir)?;
    // Best-effort permissions on the parent so it's user-only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(home.join(".sqlv")) {
            let mut perms = meta.permissions();
            perms.set_mode(0o700);
            let _ = fs::set_permissions(home.join(".sqlv"), perms);
        }
    }
    Ok(dir)
}

pub fn register(port: u16) -> io::Result<Instance> {
    let token = generate_token()?;
    let pid = std::process::id();
    let started_at = humantime::format_rfc3339_seconds_or_now();

    let info = InstanceInfo {
        pid,
        port,
        token,
        started_at,
    };
    let path = dir()?.join(format!("{pid}.json"));

    let body = serde_json::to_vec_pretty(&info).map_err(io::Error::other)?;
    fs::write(&path, &body)?;

    // Lock the file down — token must not be readable by other users.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    eprintln!("[sqlv] registered instance at {}", path.display());
    Ok(Instance { path, info })
}

fn generate_token() -> io::Result<String> {
    let mut buf = [0u8; 24];
    getrandom::getrandom(&mut buf).map_err(|e| io::Error::other(format!("getrandom: {e}")))?;
    let mut s = String::with_capacity(buf.len() * 2);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    Ok(s)
}

// Small inline RFC3339 formatter — avoids pulling in `humantime` for real.
mod humantime {
    use super::SystemTime;
    pub fn format_rfc3339_seconds_or_now() -> String {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format_unix_seconds(secs)
    }

    fn format_unix_seconds(secs: u64) -> String {
        // Civil-time conversion (gregorian) — good enough for a timestamp.
        let days = (secs / 86_400) as i64;
        let sod = secs % 86_400;
        let h = sod / 3600;
        let m = (sod / 60) % 60;
        let s = sod % 60;
        let (y, mo, d) = civil_from_days(days + 719_468);
        format!(
            "{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z",
            y = y,
            mo = mo,
            d = d,
            h = h,
            m = m,
            s = s
        )
    }

    // Howard Hinnant's proleptic Gregorian algorithm.
    fn civil_from_days(z: i64) -> (i32, u32, u32) {
        let era = if z >= 0 {
            z / 146_097
        } else {
            (z - 146_096) / 146_097
        };
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (y + if m <= 2 { 1 } else { 0 }) as i32;
        (y, m, d)
    }
}
