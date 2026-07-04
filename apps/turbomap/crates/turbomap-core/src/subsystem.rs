//! The Subsystem contract — observability as an obligation
//! (architecture §III.6, slice D2).
//!
//! Every subsystem of the map (Basemap, Terrain, Symbols, Overlays,
//! Atmosphere, …) implements [`Subsystem`]: a name, the frame-graph passes
//! it contributes, a budget report, a compact inspect-JSON snapshot of its
//! live state, and at least one debug view. Being debuggable is not optional
//! equipment — the registry meta-test (in `turbomap-golden`) fails the build
//! if a registered subsystem returns an empty contract.
//!
//! This slice lands the **observability core** of the S7 contract. The
//! remaining S7 methods arrive with their callers: `reconcile` (the Scene
//! slice channel) stays at the engine boundary until subsystem-scoped scene
//! slices exist, `tick` joins with the first `SimulationSystem` (E2), and
//! `data_needs` joins when subsystems own their `WorldLayerId`s (D3).

/// One subsystem's resource usage against its caps. `budget == 0` means the
/// dimension is unbudgeted for this subsystem (not "over budget").
#[derive(Debug, Clone, Copy, Default)]
pub struct BudgetReport {
    /// GPU/CPU cache bytes attributable to this subsystem.
    pub bytes_used: usize,
    /// The byte cap those caches evict against (0 = unbudgeted).
    pub bytes_budget: usize,
    /// Resident countable items (cache entries, markers, tubes, …) — the
    /// working-set size in the subsystem's own unit.
    pub items: usize,
}

/// How a debug view is switched on.
#[derive(Debug, Clone, Copy)]
pub enum DebugActivation {
    /// Disable the named frame-graph pass (`Map::set_pass_enabled(name,
    /// false)`) — the frame renders *without* this stage, so diffing against
    /// the full frame shows exactly what the stage contributes.
    MaskPass(&'static str),
    /// A parameter switch documented on the subsystem's API (e.g. the cloud
    /// pipeline's AOV `DebugView` via `set_cloud_params`).
    Param(&'static str),
}

/// A named, activatable way to look at one stage of a subsystem in
/// isolation. The scenario harness enumerates these (`TURBO_PASS_ISOLATE`
/// renders every `MaskPass` view automatically).
#[derive(Debug, Clone, Copy)]
pub struct DebugViewDesc {
    pub name: &'static str,
    pub description: &'static str,
    pub activation: DebugActivation,
}

/// The contract every map subsystem implements (architecture §III.6).
pub trait Subsystem {
    /// Stable, unique identifier (`"basemap"`, `"terrain"`, …).
    fn name(&self) -> &'static str;
    /// Frame-graph pass names this subsystem contributes (the `PassDesc`
    /// names in `Map::render`'s pass set).
    fn passes(&self) -> &'static [&'static str];
    /// Live resource usage vs caps.
    fn budgets(&self) -> BudgetReport;
    /// Compact JSON object describing live state. Always a valid JSON
    /// object; schema is per-subsystem and documented on the impl.
    fn inspect(&self) -> String;
    /// At least one way to visually isolate/inspect a stage of this
    /// subsystem. The registry meta-test enforces non-emptiness.
    fn debug_views(&self) -> &'static [DebugViewDesc];
}

/// Escape a dynamic string for embedding in the hand-rolled inspect JSON
/// (turbomap-core is deliberately serde-free; the consumers parse with real
/// JSON parsers). Handles quotes, backslashes and control characters.
pub(crate) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_handles_quotes_backslashes_and_controls() {
        assert_eq!(json_escape("plain"), "plain");
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
        assert_eq!(json_escape("a\nb\tc"), "a\\nb\\tc");
        assert_eq!(json_escape("\u{01}"), "\\u0001");
    }
}
