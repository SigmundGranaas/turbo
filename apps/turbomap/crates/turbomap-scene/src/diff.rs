//! Pure scene diffing: `diff(old, new) -> SceneDelta`.
//!
//! This is a total function with no renderer dependency, which is the
//! whole point — the minimal change set for any edit is unit-testable
//! without a GPU. A renderer consumes the delta to update GPU state; the
//! tests consume it to assert correctness.

use std::collections::BTreeMap;

use crate::scene::{Layer, Scene, SourceDef};

/// What happened to one keyed source between two scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceChange {
    Added(String),
    Removed(String),
    /// Same key, different definition (e.g. new GeoJSON data).
    Updated(String),
}

/// What happened to one layer between two scenes. A layer may be both
/// `Updated` (content changed) and `Moved` (order changed).
#[derive(Clone, Debug, PartialEq)]
pub enum LayerChange {
    Added { id: String, index: usize },
    Removed { id: String },
    /// Same id, different content.
    Updated { id: String },
    /// Same id and content, different stack position.
    Moved { id: String, from: usize, to: usize },
}

/// The minimal set of changes turning one scene into another.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneDelta {
    pub sources: Vec<SourceChange>,
    pub layers: Vec<LayerChange>,
    /// `Some(new)` when the scene-declared environment changed (plan C1) —
    /// the engine applies the whole block (it's a handful of scalars, so
    /// per-field deltas would be complexity without a saving).
    pub environment: Option<crate::EnvironmentDef>,
}

impl SceneDelta {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty() && self.layers.is_empty() && self.environment.is_none()
    }
}

/// Diff `old` into `new`, producing the minimal [`SceneDelta`].
pub fn diff(old: &Scene, new: &Scene) -> SceneDelta {
    SceneDelta {
        sources: diff_sources(&old.sources, &new.sources),
        layers: diff_layers(&old.layers, &new.layers),
        environment: (old.environment != new.environment)
            .then(|| new.environment.clone()),
    }
}

fn diff_sources(
    old: &BTreeMap<String, SourceDef>,
    new: &BTreeMap<String, SourceDef>,
) -> Vec<SourceChange> {
    let mut out = Vec::new();
    for (key, def) in new {
        match old.get(key) {
            None => out.push(SourceChange::Added(key.clone())),
            Some(prev) if prev != def => out.push(SourceChange::Updated(key.clone())),
            Some(_) => {}
        }
    }
    for key in old.keys() {
        if !new.contains_key(key) {
            out.push(SourceChange::Removed(key.clone()));
        }
    }
    out
}

fn diff_layers(old: &[Layer], new: &[Layer]) -> Vec<LayerChange> {
    let old_index: BTreeMap<&str, (usize, &Layer)> =
        old.iter().enumerate().map(|(i, l)| (l.id(), (i, l))).collect();
    let new_index: BTreeMap<&str, (usize, &Layer)> =
        new.iter().enumerate().map(|(i, l)| (l.id(), (i, l))).collect();

    let mut out = Vec::new();

    // Removed: in old, gone from new.
    for l in old {
        if !new_index.contains_key(l.id()) {
            out.push(LayerChange::Removed {
                id: l.id().to_string(),
            });
        }
    }
    // Added: in new, absent from old.
    for (i, l) in new.iter().enumerate() {
        if !old_index.contains_key(l.id()) {
            out.push(LayerChange::Added {
                id: l.id().to_string(),
                index: i,
            });
        }
    }

    // Common ids, in each scene's order — the subsequences we reconcile.
    let common_old: Vec<&str> = old
        .iter()
        .map(|l| l.id())
        .filter(|id| new_index.contains_key(id))
        .collect();
    let common_new: Vec<&str> = new
        .iter()
        .map(|l| l.id())
        .filter(|id| old_index.contains_key(id))
        .collect();

    // Ids whose relative order is preserved — they don't need moving.
    let stable = longest_common_subsequence(&common_old, &common_new);

    for id in &common_new {
        let (old_i, old_layer) = old_index[id];
        let (new_i, new_layer) = new_index[id];
        if old_layer != new_layer {
            out.push(LayerChange::Updated {
                id: (*id).to_string(),
            });
        }
        if !stable.contains(id) {
            out.push(LayerChange::Moved {
                id: (*id).to_string(),
                from: old_i,
                to: new_i,
            });
        }
    }

    out
}

/// Ids appearing in a longest common subsequence of two id sequences.
/// Ids *not* in it are the minimal set that must move to reconcile order.
fn longest_common_subsequence<'a>(a: &[&'a str], b: &[&'a str]) -> std::collections::BTreeSet<&'a str> {
    let (n, m) = (a.len(), b.len());
    // dp[i][j] = LCS length of a[i..] and b[j..].
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    // Reconstruct one LCS, collecting its ids.
    let mut set = std::collections::BTreeSet::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            set.insert(a[i]);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    set
}
