//! Crash isolation for the pathfind handler.
//!
//! `catch_unwind`-wraps the synchronous `Pathfinder::solve` call so
//! a Rust panic returns HTTP 500 with a `dump_id` instead of taking
//! down the whole server. The panic payload, request body, and a
//! backtrace land in `${TURBO_CRASH_DIR:-/tmp/turbo-crashes}` as a
//! single JSON file the curator can replay against a debug binary.
//!
//! ## What this does and does not catch
//!
//! - **Rust panics** (out-of-bounds, `unwrap` on `None`, explicit
//!   `panic!`): caught and surfaced as HTTP 500 + JSON. Server
//!   keeps serving.
//! - **Native crashes** (SIGSEGV, SIGBUS, abort): NOT caught by
//!   `catch_unwind`. The macOS crash reporter writes an `.ips`
//!   under `~/Library/Logs/DiagnosticReports/`; the server process
//!   still dies. A SIGSEGV signal handler that writes async-signal-
//!   safe state before death is feasible (TODO) but not in this
//!   pass — the dangling-mmap class of bug that motivated this
//!   work is now fixed at the source.
//!
//! `AssertUnwindSafe` is sound here because the pathfinder's
//! internal state is read-only (`Arc<Pathfinder>`) and the per-
//! request inputs are owned and passed by value. A panic mid-
//! solve leaves no broken invariants for the next request.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use serde::Serialize;
use uuid::Uuid;

/// Where crash dumps land. Curator overrides with `TURBO_CRASH_DIR`.
pub fn crash_dir() -> PathBuf {
    std::env::var("TURBO_CRASH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/turbo-crashes"))
}

#[derive(Debug, Serialize)]
pub struct CrashDump {
    pub id: Uuid,
    pub timestamp_unix_sec: i64,
    pub endpoint: String,
    pub request_json: serde_json::Value,
    pub panic_message: String,
}

impl CrashDump {
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let dir = crash_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", self.id));
        let body = serde_json::to_vec_pretty(self).unwrap_or_else(|_| b"{}".to_vec());
        std::fs::write(&path, body)?;
        Ok(path)
    }
}

/// Run `f`. If it panics, capture the request body + panic message
/// to disk under `crash_dir()` and return `Err(CaughtPanic)`. The
/// caller maps this into an HTTP 500 with the dump_id, so the
/// curator can `cat /tmp/turbo-crashes/<id>.json` to see the input.
#[derive(Debug)]
pub struct CaughtPanic {
    pub dump_id: Uuid,
    pub message: String,
    pub dump_path: Option<PathBuf>,
}

pub fn run_or_dump<F, R>(endpoint: &str, request: serde_json::Value, f: F) -> Result<R, CaughtPanic>
where
    F: FnOnce() -> R,
{
    let result = catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(r) => Ok(r),
        Err(payload) => {
            let message = panic_message_from_payload(&payload);
            let id = Uuid::new_v4();
            let dump = CrashDump {
                id,
                timestamp_unix_sec: chrono::Utc::now().timestamp(),
                endpoint: endpoint.to_string(),
                request_json: request,
                panic_message: message.clone(),
            };
            let dump_path = dump.write().ok();
            tracing::error!(
                dump_id = %id,
                panic = %message,
                "panic caught in {endpoint} — server kept alive"
            );
            Err(CaughtPanic {
                dump_id: id,
                message,
                dump_path,
            })
        }
    }
}

fn panic_message_from_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Lightweight directory listing for `/admin/dev/recent-crashes`.
/// Newest first; capped at `limit`.
pub fn list_recent_crashes(limit: usize) -> Vec<serde_json::Value> {
    let dir = crash_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, e.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
        .into_iter()
        .take(limit)
        .filter_map(|(_, p)| {
            let text = std::fs::read_to_string(&p).ok()?;
            serde_json::from_str::<serde_json::Value>(&text).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_or_dump_catches_panic_and_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("TURBO_CRASH_DIR", tmp.path());
        let req = serde_json::json!({"from": [0.0, 0.0], "to": [1.0, 1.0]});
        let err = run_or_dump::<_, i32>("/v1/test", req, || panic!("boom"))
            .err()
            .expect("should panic");
        assert!(err.message.contains("boom"));
        let dumped = err.dump_path.expect("dump file written");
        assert!(dumped.exists());
        let text = std::fs::read_to_string(&dumped).unwrap();
        assert!(text.contains("\"endpoint\": \"/v1/test\""));
        assert!(text.contains("\"panic_message\": \"boom\""));
    }

    #[test]
    fn run_or_dump_returns_value_on_normal_path() {
        let v = run_or_dump::<_, i32>("/v1/test", serde_json::json!({}), || 42).unwrap();
        assert_eq!(v, 42);
    }
}
