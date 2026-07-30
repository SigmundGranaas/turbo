//! Cost layers — the extension point.
//!
//! A `CostLayer` reports two things about traversing a piece of
//! terrain:
//!
//! - **`cell_cost(x, y, profile)`** — a `CellCost { multiplier,
//!   refused }`. Multipliers compose by *product* (so a 2× slope on
//!   top of a 1.5× marsh becomes 3×). A non-`None` `refused` vetoes
//!   the cell outright; the first refusal in layer order wins.
//!
//! - **`edge_multiplier(edge, profile)`** — a scalar applied on top
//!   of the graph's baked per-profile cost. Lets a layer favour or
//!   discourage individual graph edges without rebuilding the
//!   `norway.graph` artifact. Premade tracks live here.
//!
//! ## How to add a new layer
//!
//! ```text
//! pub struct MarshLayer { pub mask: Arc<MarshMask>, pub weight: f32 }
//!
//! impl CostLayer for MarshLayer {
//!     fn name(&self) -> &'static str { "marsh" }
//!     fn cell_cost(&self, x: f64, y: f64, _p: Profile) -> CellCost {
//!         if self.mask.is_marsh(x, y) {
//!             CellCost::multiplier(1.0 + 1.5 * self.weight)
//!         } else { CellCost::default() }
//!     }
//! }
//! ```
//!
//! Register the layer with the `Pathfinder` at boot and it's
//! automatically picked up by both the off-trail mesh and the
//! graph router. No format changes, no migrations.
//!
//! ## Composition rules
//!
//! Per cell:
//!   total_multiplier  = ∏ layer.cell_cost(x,y).multiplier
//!   refused-by        = the first layer whose `refused` is Some
//!
//! Per edge:
//!   total_edge_mult   = ∏ layer.edge_multiplier(edge)
//!   final edge cost   = baked_cost(edge) × total_edge_mult
//!   forbidden if      = any factor is infinite or NaN


use turbo_tiles_graph::{EdgeRecord, Profile};

/// What a layer reports about a single mesh cell.
#[derive(Debug, Clone, Copy)]
pub struct CellCost {
    /// Multiplier on the base unit cost (1.0 = nominal terrain).
    /// Combined multiplicatively across layers.
    pub multiplier: f32,
    /// If `Some`, this cell is refused — Theta\* will treat it as
    /// impassable. The string is a short label for debugging
    /// ("water", "glacier", "out-of-coverage"…) and surfaces via
    /// `/v1/debug/pathfind/inspect` so curators can see *why* a
    /// cell was rejected.
    pub refused: Option<&'static str>,
}

impl Default for CellCost {
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            refused: None,
        }
    }
}

impl CellCost {
    pub fn multiplier(m: f32) -> Self {
        Self {
            multiplier: m,
            refused: None,
        }
    }
    pub fn refused(reason: &'static str) -> Self {
        Self {
            multiplier: 1.0,
            refused: Some(reason),
        }
    }
}

/// Cost layer trait. Implementors live in this crate (built-ins)
/// or downstream — they're plain Rust structs, no FFI, no IPC.
pub trait CostLayer: Send + Sync {
    /// Stable lower-case identifier. Used as the key in the
    /// per-request `layer_weights` config and in debug output.
    fn name(&self) -> &'static str;

    /// Default 1.0 multiplier when the layer isn't disabled but has
    /// no opinion about a cell. Override when the layer always reads.
    fn cell_cost(&self, _x: f64, _y: f64, _profile: Profile) -> CellCost {
        CellCost::default()
    }

    /// Override to bias graph-edge selection. Return `f32::INFINITY`
    /// to forbid traversal entirely (e.g. a winter-closed road for
    /// the `foot` profile in summer mode).
    fn edge_multiplier(&self, _edge: &EdgeRecord, _profile: Profile) -> f32 {
        1.0
    }

    /// Direction-aware modifier for a mesh edge from `(fx, fy)` to
    /// `(tx, ty)` (EPSG:25833 metres). Multiplied onto the symmetric
    /// `cell_cost` average when the off-trail solver evaluates an
    /// edge. Default `1.0` keeps existing layers symmetric.
    ///
    /// Implementors that need direction information (e.g. uphill
    /// vs downhill vs traverse on a slope, lee vs windward aspect)
    /// override this. The `cell_cost` impl can still return `1.0`
    /// — the two are independent contributions.
    fn edge_cost_modifier(&self, _fx: f64, _fy: f64, _tx: f64, _ty: f64, _profile: Profile) -> f32 {
        1.0
    }

    /// Does this layer have authoritative data at `(x, y)`?
    ///
    /// The pathfinder uses this to detect "user clicked outside
    /// our coverage" — if no layer covers either endpoint *and*
    /// no graph anchor is within reach, the request is refused
    /// instead of falling back to a uniform-cost mesh that would
    /// produce a meaningless straight-line "path".
    ///
    /// Default `false` — implementors must explicitly say which
    /// points they know about. Edge-only layers
    /// (`PreferredEdgeLayer`, `MarkingLayer`) leave the default;
    /// they don't describe terrain at a point.
    fn covers(&self, _x: f64, _y: f64) -> bool {
        false
    }
}

/// Helper: compose a stack of layers into a single per-cell answer.
/// Respects per-layer weights when supplied — `weight = 0.0` is
/// "disabled", `1.0` is "as-is", values in between scale the
// `compose_cell` / `compose_edge` (the legacy multiplicative composers)
// were deleted in B1. E3 proved the multiplier channel never reached the
// solver: perturbing a legacy layer's multiplier by 37x moved neither the
// corpus geometry hash nor the DEM lookup count, while a 1% change to a
// native contributor moved both. Their only live consumer was the inspect
// endpoint, now served by `Pathfinder::inspect_cell` off the contributor
// stack.


#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(&'static str, f32);
    impl CostLayer for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }
        fn cell_cost(&self, _x: f64, _y: f64, _p: Profile) -> CellCost {
            CellCost::multiplier(self.1)
        }
    }

    struct Refuser(&'static str);
    impl CostLayer for Refuser {
        fn name(&self) -> &'static str {
            self.0
        }
        fn cell_cost(&self, _x: f64, _y: f64, _p: Profile) -> CellCost {
            CellCost::refused("test")
        }
    }

    fn unit_weight(_: &str) -> f32 {
        1.0
    }




}
