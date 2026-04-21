use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde_json::json;

use crate::cli::{PushArgs, PushOpenArgs};
use crate::exit::{Failure, EXIT_OTHER};
use crate::output;
use crate::params::parse_params;

const TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_HEADER: &str = "X-Sqlv-Token";

#[derive(Debug, Deserialize, Clone)]
struct InstanceInfo {
    #[allow(dead_code)]
    pid: u32,
    port: u16,
    token: String,
    #[allow(dead_code)]
    #[serde(default)]
    started_at: String,
}

pub fn query(args: PushArgs, force_json: bool) -> Result<(), Failure> {
    let params = parse_params(&args.params)?;
    let json_params: Vec<serde_json::Value> = params.iter().map(value_to_json).collect();

    let body = json!({
        "sql": args.sql,
        "params": json_params,
        "limit": args.limit,
        "offset": args.offset,
        "mode": if args.run { "run" } else { "auto" },
    });

    let resp = send_request(args.port, "/query", &body)?;
    output::emit(&resp, force_json);
    Ok(())
}

pub fn open(args: PushOpenArgs, force_json: bool) -> Result<(), Failure> {
    // Resolve relative to the CLI's CWD before crossing the HTTP boundary —
    // the desktop backend's process lives in its own (very different) cwd, so
    // a bare "samples/foo.db" argument would resolve against that instead.
    // The agent/user types paths relative to their shell; make that Just Work.
    let abs = absolutize(&args.path);
    let body = json!({
        "path": abs,
        "read_only": !args.write,
    });
    let resp = send_request(args.port, "/open", &body)?;
    output::emit(&resp, force_json);
    Ok(())
}

/// Make a path string absolute relative to the current working directory.
/// Does NOT require the file to exist (opening a missing file still flows
/// through the normal error path with a clear message). Does NOT follow
/// symlinks; that's fine here — sqlite opens whatever the OS points at.
fn absolutize(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(p).to_string_lossy().into_owned(),
        Err(_) => path.to_string(),
    }
}

fn send_request(
    port_override: Option<u16>,
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, Failure> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(250))
        .timeout(TIMEOUT)
        .build();

    // Build the list of candidate instances. User-forced port skips
    // discovery entirely (still best-effort, no token). Otherwise we
    // enumerate `~/.sqlv/instances/`, probe each via /health, and
    // prune anything dead from disk so the directory doesn't grow
    // without bound between desktop restarts.
    let candidates: Vec<InstanceInfo> = match port_override {
        Some(port) => vec![InstanceInfo {
            pid: 0,
            port,
            token: String::new(),
            started_at: String::new(),
        }],
        None => live_instances(),
    };

    if candidates.is_empty() {
        return Err(Failure::new(
            "io",
            "no running desktop app found under ~/.sqlv/instances/. \
             Start the app (`bunx tauri dev` in apps/desktop, or the installed build) and retry."
                .to_string(),
            EXIT_OTHER,
        ));
    }

    // Since `live_instances()` already filtered to endpoints that
    // answered `/health`, any non-2xx response here is legitimately
    // from a sqlv backend — report it as-is. Network errors trying to
    // reach one endpoint still fall through to the next (a race could
    // kill the instance between probe and send).
    let mut last_err: Option<String> = None;
    for inst in candidates {
        let url = format!("http://127.0.0.1:{}{path}", inst.port);
        let mut req = agent.post(&url);
        if !inst.token.is_empty() {
            req = req.set(AUTH_HEADER, &inst.token);
        }
        match req.send_json(body) {
            Ok(resp) => {
                let v: serde_json::Value = resp
                    .into_json()
                    .map_err(|e| Failure::new("io", e.to_string(), EXIT_OTHER))?;
                return Ok(v);
            }
            Err(ureq::Error::Status(code, resp)) => {
                let v: serde_json::Value = resp.into_json().unwrap_or_else(
                    |_| json!({"error": {"code": "other", "message": format!("HTTP {code}")}}),
                );
                return Err(failure_from_http(code, &v));
            }
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        }
    }

    Err(Failure::new(
        "io",
        format!(
            "could not reach any registered desktop instance. \
             Is the desktop app running? Details: {}",
            last_err.unwrap_or_else(|| "no response".into())
        ),
        EXIT_OTHER,
    ))
}

/// Return only instances that currently answer `GET /health`, sorted
/// newest-first (by file mtime, which is set when the desktop process
/// registered itself). Any file whose port doesn't respond is deleted
/// as we go — that's how `~/.sqlv/instances/` stays bounded between
/// desktop restarts even when the app is killed ungracefully.
fn live_instances() -> Vec<InstanceInfo> {
    let probe_agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(150))
        .timeout(Duration::from_millis(250))
        .build();

    let candidates = read_instance_dir();
    let mut alive = Vec::with_capacity(candidates.len());
    for (path, info) in candidates {
        let url = format!("http://127.0.0.1:{}/health", info.port);
        match probe_agent.get(&url).call() {
            Ok(resp) if resp.status() == 200 => alive.push(info),
            _ => {
                // Dead endpoint — prune its record so subsequent pushes
                // don't scan it again.
                let _ = fs::remove_file(&path);
            }
        }
    }
    alive
}

/// Read `~/.sqlv/instances/*.json` and return parsed entries paired
/// with their on-disk paths, sorted by mtime descending so the
/// most-recently-registered instance wins the race. Missing or
/// unreadable files are silently skipped.
fn read_instance_dir() -> Vec<(PathBuf, InstanceInfo)> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let dir = home.join(".sqlv").join("instances");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out: Vec<(SystemTime, PathBuf, InstanceInfo)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(info) = serde_json::from_str::<InstanceInfo>(&body) else {
            continue;
        };
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        out.push((mtime, path, info));
    }
    // Newest first — Reverse gives descending mtime order.
    out.sort_by_key(|row| std::cmp::Reverse(row.0));
    out.into_iter().map(|(_, p, i)| (p, i)).collect()
}

fn failure_from_http(status: u16, body: &serde_json::Value) -> Failure {
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .unwrap_or("other");
    let message = body
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("unknown error")
        .to_string();

    let (static_code, exit) = match code {
        "not_found" => ("not_found", crate::exit::EXIT_NOT_FOUND),
        "readonly" => ("readonly", crate::exit::EXIT_READONLY),
        "sql" => ("sql", crate::exit::EXIT_SQL),
        "invalid" | "usage" => ("usage", crate::exit::EXIT_USAGE),
        "not_open" => ("not_open", crate::exit::EXIT_OTHER),
        _ => ("other", crate::exit::EXIT_OTHER),
    };
    let _ = status;
    Failure::new(static_code, message, exit)
}

fn value_to_json(v: &sqlv_core::Value) -> serde_json::Value {
    use sqlv_core::Value::*;
    match v {
        Null => serde_json::Value::Null,
        Integer(i) => serde_json::Value::from(*i),
        Real(f) => serde_json::Value::from(*f),
        Text(s) => serde_json::Value::from(s.clone()),
        Blob(b) => json!({ "$blob_base64": b64_encode(b) }),
    }
}

fn b64_encode(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for c in chunks.by_ref() {
        let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | (c[2] as u32);
        for shift in [18, 12, 6, 0] {
            out.push(A[((n >> shift) & 0x3f) as usize] as char);
        }
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(A[((n >> 18) & 0x3f) as usize] as char);
            out.push(A[((n >> 12) & 0x3f) as usize] as char);
            out.push_str("==");
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(A[((n >> 18) & 0x3f) as usize] as char);
            out.push(A[((n >> 12) & 0x3f) as usize] as char);
            out.push(A[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_input_passes_through() {
        assert_eq!(absolutize("/tmp/foo.db"), "/tmp/foo.db");
    }

    #[test]
    fn relative_input_is_joined_with_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let expected = cwd.join("samples/foo.db").to_string_lossy().into_owned();
        assert_eq!(absolutize("samples/foo.db"), expected);
    }

    #[test]
    fn dot_slash_is_joined() {
        let cwd = std::env::current_dir().unwrap();
        let expected = cwd.join("./x.db").to_string_lossy().into_owned();
        assert_eq!(absolutize("./x.db"), expected);
    }

    #[test]
    fn absolutize_does_not_require_existence() {
        // The path clearly doesn't exist; absolutize must still succeed.
        let out = absolutize("no-such/path/ever.sqlite");
        assert!(std::path::Path::new(&out).is_absolute());
    }
}
