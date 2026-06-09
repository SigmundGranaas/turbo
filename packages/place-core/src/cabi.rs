//! C ABI for the server's P/Invoke path (JSON in / JSON out).
//!
//! Compiled only with `--features cabi`. The .NET `Turbo.Places` service calls
//! these instead of UniFFI: the server already speaks JSON everywhere, so a
//! JSON-string boundary avoids all struct marshalling.
//!
//! Two ABIs:
//!   - `*_default` — uses the ruleset embedded at build time (parsed once via
//!     `OnceLock`); the simple path.
//!   - `place_core_engine_*` — a handle bound to a ruleset supplied at runtime
//!     (e.g. a hot-loaded version from `GET /api/places/ruleset/{v}` or an
//!     offline bundle). Construct once with `place_core_engine_new`, reuse, and
//!     release with `place_core_engine_free`.
//!
//! Ownership: returned strings must be released with [`place_core_string_free`];
//! engine handles with [`place_core_engine_free`].

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

use crate::{forward_search, reverse_geocode, ReverseInput, Ruleset, SearchCandidate};

/// A ruleset-bound engine handle. Opaque to the foreign caller.
pub struct Engine {
    ruleset: Ruleset,
}

/// The build-time ruleset, parsed once.
fn default_ruleset() -> &'static Ruleset {
    static RULESET: OnceLock<Ruleset> = OnceLock::new();
    RULESET.get_or_init(Ruleset::load_default)
}

unsafe fn read<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        CStr::from_ptr(p).to_str().ok()
    }
}

fn give(s: String) -> *mut c_char {
    // Unwrap is safe: serde_json never emits interior NULs.
    CString::new(s).unwrap_or_default().into_raw()
}

fn reverse_with(ruleset: &Ruleset, input_json: *const c_char) -> *mut c_char {
    let out = unsafe { read(input_json) }
        .and_then(|s| serde_json::from_str::<ReverseInput>(s).ok())
        .and_then(|input| reverse_geocode(ruleset, &input))
        .and_then(|d| serde_json::to_string(&d).ok())
        .unwrap_or_else(|| "null".to_string());
    give(out)
}

fn search_with(
    ruleset: &Ruleset,
    query: *const c_char,
    candidates_json: *const c_char,
) -> *mut c_char {
    let query = unsafe { read(query) }.unwrap_or("");
    let candidates: Vec<SearchCandidate> = unsafe { read(candidates_json) }
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let hits = forward_search(ruleset, query, &candidates);
    give(serde_json::to_string(&hits).unwrap_or_else(|_| "[]".to_string()))
}

// ── Engine handle API ───────────────────────────────────────────────────────

/// Build an engine bound to `ruleset_json`. Returns null if it doesn't parse.
///
/// # Safety
/// `ruleset_json` must be a valid NUL-terminated UTF-8 C string or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_engine_new(ruleset_json: *const c_char) -> *mut Engine {
    match read(ruleset_json).and_then(|s| Ruleset::from_json(s).ok()) {
        Some(ruleset) => Box::into_raw(Box::new(Engine { ruleset })),
        None => std::ptr::null_mut(),
    }
}

/// Release an engine handle.
///
/// # Safety
/// `engine` must be a handle from [`place_core_engine_new`], or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_engine_free(engine: *mut Engine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// A null engine falls back to the embedded ruleset — so the `_default` shims
/// and a null handle behave identically.
unsafe fn engine_ruleset<'a>(engine: *mut Engine) -> &'a Ruleset {
    if engine.is_null() {
        default_ruleset()
    } else {
        &(*engine).ruleset
    }
}

/// # Safety
/// `engine` must be a valid handle or null; `input_json` a valid C string or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_engine_reverse(
    engine: *mut Engine,
    input_json: *const c_char,
) -> *mut c_char {
    reverse_with(engine_ruleset(engine), input_json)
}

/// # Safety
/// `engine` must be a valid handle or null; both strings valid C strings or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_engine_search(
    engine: *mut Engine,
    query: *const c_char,
    candidates_json: *const c_char,
) -> *mut c_char {
    search_with(engine_ruleset(engine), query, candidates_json)
}

// ── Default-ruleset shims ─────────────────────────────────────────────────────

/// Reverse-geocode against the embedded ruleset.
///
/// # Safety
/// `input_json` must be a valid NUL-terminated UTF-8 C string or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_reverse_default(input_json: *const c_char) -> *mut c_char {
    reverse_with(default_ruleset(), input_json)
}

/// Forward-search against the embedded ruleset.
///
/// # Safety
/// Both arguments must be valid NUL-terminated UTF-8 C strings or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_search_default(
    query: *const c_char,
    candidates_json: *const c_char,
) -> *mut c_char {
    search_with(default_ruleset(), query, candidates_json)
}

/// Release a string returned by this ABI.
///
/// # Safety
/// `p` must be a pointer previously returned by this module, or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_string_free(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn reverse(engine: *mut Engine, input: &str) -> String {
        let c = CString::new(input).unwrap();
        let out = place_core_engine_reverse(engine, c.as_ptr());
        let s = CStr::from_ptr(out).to_str().unwrap().to_owned();
        place_core_string_free(out);
        s
    }

    /// The engine handle must actually run the ruleset it was given — a
    /// runtime-supplied ruleset changes the verdict. (Impossible against a
    /// default-only ABI; this is the P0.2 red test.)
    #[test]
    fn engine_honours_a_runtime_ruleset() {
        // A peak 50 m away. Embedded ruleset (peak `on` ≤ 100 m) ⇒ "on".
        let input = r#"{"toponyms":[{"name":"Topp","kind":"fjelltopp","distance_m":50.0,"status":"aktiv"}]}"#;

        unsafe {
            let default_ptr = CString::new(input).unwrap();
            let default_out = place_core_reverse_default(default_ptr.as_ptr());
            let default_str = CStr::from_ptr(default_out).to_str().unwrap().to_owned();
            place_core_string_free(default_out);
            assert!(default_str.contains("\"on\""), "default: {default_str}");

            // Shrink the first rule (peak `on`) to ≤ 5 m: 50 m no longer "on".
            let mut v: serde_json::Value =
                serde_json::from_str(include_str!("../ruleset.v1.json")).unwrap();
            v["rules"][0]["max_m"] = serde_json::json!(5.0);
            let modified = serde_json::to_string(&v).unwrap();

            let c = CString::new(modified).unwrap();
            let engine = place_core_engine_new(c.as_ptr());
            assert!(!engine.is_null(), "modified ruleset should parse");
            let modified_out = reverse(engine, input);
            place_core_engine_free(engine);

            assert!(!modified_out.contains("\"on\""), "modified: {modified_out}");
            assert!(
                modified_out.contains("\"closeTo\""),
                "modified: {modified_out}"
            );
        }
    }

    #[test]
    fn engine_new_rejects_invalid_ruleset() {
        unsafe {
            let bad = CString::new("{ not a ruleset").unwrap();
            assert!(place_core_engine_new(bad.as_ptr()).is_null());
        }
    }
}
