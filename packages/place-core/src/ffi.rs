//! UniFFI surface — the binding boundary for Kotlin / Swift / Python.
//!
//! Compiled only with `--features uniffi`. The exported [`PlaceEngine`] holds a
//! parsed ruleset (so foreign callers construct it once and reuse it) and
//! exposes the two pure entry points. Everything it returns is a UniFFI
//! `Record` defined in [`crate::model`], so the bindings map 1:1 onto the
//! clients' own types.

use std::fmt;
use std::sync::Arc;

use crate::model::{LocationDescription, ReverseInput, SearchCandidate, SearchHit};
use crate::ruleset::Ruleset;

/// Errors surfaced across the FFI boundary.
#[derive(Debug, uniffi::Error)]
pub enum EngineError {
    /// `from_ruleset_json` was given a ruleset that didn't parse.
    InvalidRuleset { message: String },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::InvalidRuleset { message } => write!(f, "invalid ruleset: {message}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// A reusable engine bound to one ruleset version. Construct once, call many.
#[derive(uniffi::Object)]
pub struct PlaceEngine {
    ruleset: Ruleset,
}

#[uniffi::export]
impl PlaceEngine {
    /// Engine using the ruleset embedded in the library (`ruleset.v1.json`).
    #[uniffi::constructor]
    pub fn with_default_ruleset() -> Arc<Self> {
        Arc::new(Self {
            ruleset: Ruleset::load_default(),
        })
    }

    /// Engine using a ruleset fetched at runtime (e.g. from
    /// `GET /api/places/ruleset/{version}` or an offline bundle).
    #[uniffi::constructor]
    pub fn from_ruleset_json(json: String) -> Result<Arc<Self>, EngineError> {
        Ruleset::from_json(&json)
            .map(|ruleset| Arc::new(Self { ruleset }))
            .map_err(|e| EngineError::InvalidRuleset {
                message: e.to_string(),
            })
    }

    /// The ruleset version this engine is running.
    pub fn ruleset_version(&self) -> String {
        self.ruleset.version.clone()
    }

    /// Reverse-geocode a gathered [`ReverseInput`] to a [`LocationDescription`].
    pub fn reverse_geocode(&self, input: ReverseInput) -> Option<LocationDescription> {
        crate::reverse_geocode(&self.ruleset, &input)
    }

    /// Order forward-search candidates for `query` into ranked [`SearchHit`]s.
    pub fn forward_search(
        &self,
        query: String,
        candidates: Vec<SearchCandidate>,
    ) -> Vec<SearchHit> {
        crate::forward_search(&self.ruleset, &query, &candidates)
    }
}
