//! C ABI for the server's P/Invoke path (JSON in / JSON out).
//!
//! Compiled only with `--features cabi`. The .NET `Turbo.Places` service calls
//! these instead of UniFFI: the server already speaks JSON everywhere, so a
//! JSON-string boundary avoids all struct marshalling. Each call uses the
//! embedded `ruleset.v1.json`.
//!
//! Ownership: returned pointers are heap-allocated C strings the caller must
//! release with [`place_core_string_free`].

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::{forward_search, reverse_geocode, ReverseInput, Ruleset, SearchCandidate};

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

/// Reverse-geocode a JSON [`ReverseInput`] → JSON `LocationDescription` (or the
/// literal `null`). Returns `null` JSON on any parse/empty result.
///
/// # Safety
/// `input_json` must be a valid NUL-terminated UTF-8 C string or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_reverse_default(input_json: *const c_char) -> *mut c_char {
    let ruleset = Ruleset::load_default();
    let out = read(input_json)
        .and_then(|s| serde_json::from_str::<ReverseInput>(s).ok())
        .and_then(|input| reverse_geocode(&ruleset, &input))
        .and_then(|d| serde_json::to_string(&d).ok())
        .unwrap_or_else(|| "null".to_string());
    give(out)
}

/// Forward-search: rank a JSON array of [`SearchCandidate`] for `query` → JSON
/// array of `SearchHit`.
///
/// # Safety
/// Both arguments must be valid NUL-terminated UTF-8 C strings or null.
#[no_mangle]
pub unsafe extern "C" fn place_core_search_default(
    query: *const c_char,
    candidates_json: *const c_char,
) -> *mut c_char {
    let ruleset = Ruleset::load_default();
    let query = read(query).unwrap_or("");
    let candidates: Vec<SearchCandidate> = read(candidates_json)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let hits = forward_search(&ruleset, query, &candidates);
    give(serde_json::to_string(&hits).unwrap_or_else(|_| "[]".to_string()))
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
