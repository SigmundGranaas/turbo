//! **L1 — the routing engine's model.** The shapes it reasons *about*,
//! not the sources it reads *from*.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §3.
//!
//! The engine's cost model and solvers used to hold `Arc<Dem>` — the
//! concrete mmap'd Norwegian artifact type. That made "swap the DEM
//! source or resolution" mean editing `turbo-tiles-elev`, the crate
//! every contributor imports, and it is what blocked region packs,
//! in-memory fixtures, and any non-Norwegian terrain.
//!
//! # The invariant
//!
//! This crate has **zero dependencies**. That is not tidiness, it is the
//! enforcement mechanism: a port that cannot name a file cannot acquire
//! one. Everything about acquisition, decoding, projection, config and
//! source selection lives above, in the composition layer (L5) and the
//! adapters (L3) — `turbo-geodata-artifacts`, `-pack`, `-memory`.
//!
//! # Shapes, not sources
//!
//! The names are deliberate. "Source" implies acquisition; "field" names
//! a queryable shape. A game engine's terrain chunk **is** a heightfield;
//! it is not a source of one. That distinction is the whole point — it is
//! what makes porting this engine to a game, a simulator, or a different
//! country a matter of writing one `impl`, not editing the engine.
//!
//! # Why `dyn`, not generics
//!
//! The design originally required "generics in the hot loop, `dyn` at the
//! edges", with monomorphisation as the mitigation for dispatch cost —
//! called out as the largest technical bet in the rationale. Experiment
//! E2 measured it against the real DEM: `sample` costs **135 ns** and
//! `slope_aspect` **225 ns**, both dominated by the tile lookup, while
//! concrete / `dyn` / monomorphised-generic land within half a nanosecond
//! of each other and disagree in sign. Derived solve-level penalty was
//! −0.014% to −0.059% against a 2% budget.
//!
//! The rule is dropped. `Arc<dyn Trait>` throughout, no generic
//! parameters threaded through the engine, one fewer invariant to
//! enforce forever.

#![forbid(unsafe_code)]

/// A point in the engine's planar frame. **Metres, always.**
///
/// There is deliberately no `GeoPoint` and no `Projection` anywhere in
/// the model: the engine works in one planar frame and never learns
/// which one. Geographic conversion is a composition-layer concern
/// (`turbo-geo-frame`, L5). That is what lets the game-engine case need
/// no projection at all, rather than an identity stub.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A planar axis-aligned bounding box, in the same metric frame as
/// [`Point`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Extent {
    #[inline]
    pub const fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }

    #[inline]
    pub fn width_m(&self) -> f64 {
        self.max_x - self.min_x
    }

    #[inline]
    pub fn height_m(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Local terrain orientation: slope from horizontal, aspect clockwise
/// from north.
///
/// The engine consumes this; how the field derives it (finite
/// differences over a tile, an analytic surface, a game's precomputed
/// normal map) is entirely the implementation's business.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SlopeAspect {
    pub slope_deg: f32,
    pub aspect_deg: f32,
}

/// A continuous scalar field of terrain height over the plane.
///
/// Nothing here mentions files, tiles, formats, compression, or
/// resolution *sources*. Anything that can answer "how high is it here"
/// can drive the engine.
///
/// # Two queries, not one
///
/// [`covers`](Heightfield::covers) and [`height_at`](Heightfield::height_at)
/// are separate on purpose. The artifact API they replaced returned
/// `Result<Option<f32>>`, and callers used its two failure modes to mean
/// different things — `is_ok()` for "inside the extent, even if nodata"
/// and `ok().flatten()` for "has a value here". Conflating them is what
/// made the coverage defect (audit finding D1) easy to write: an
/// advisory landcover mask extending past the DEM granted *routing*
/// coverage at points with no elevation at all, and the solver then
/// built a uniform-cost mesh and returned a straight line — which the
/// code itself calls "semantically a lie".
///
/// So the port splits them: `covers` answers **authority**, `height_at`
/// answers **value**.
pub trait Heightfield: Send + Sync {
    /// Height in metres, or `None` for no data at this point — whether
    /// because the point is outside the field or because the field has a
    /// hole there. Use [`Self::covers`] to distinguish.
    fn height_at(&self, p: Point) -> Option<f32>;

    /// Is this point inside the field's authoritative extent?
    ///
    /// `true` for a nodata hole *inside* coverage: the field is
    /// authoritative that it does not know. `false` only outside the
    /// extent entirely. This is the distinction routing feasibility
    /// depends on.
    fn covers(&self, p: Point) -> bool;

    /// Local slope and aspect, or `None` where undefined.
    fn slope_aspect_at(&self, p: Point) -> Option<SlopeAspect>;

    /// The field's bounding extent.
    fn extent(&self) -> Extent;

    /// Intrinsic sample spacing in metres.
    ///
    /// Corridor sizing uses this to avoid asking finer questions than
    /// the data can answer: a 10 m field cannot honestly resolve a 2 m
    /// grid, and pretending otherwise costs work without adding
    /// information.
    fn resolution_m(&self) -> f32;
}

/// Whether a cost contributor is load-bearing for routing feasibility.
///
/// The distinction is not decoration — it is the fix for audit finding
/// D1. Routing coverage is the **intersection of `Required`**
/// contributors, never the union of everything that happens to be able
/// to answer a question at a point. A landcover mask that extends past
/// the elevation data can tell you there is forest there; it cannot tell
/// you the terrain is routable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// Without this contributor's data the point is not routable.
    Required,
    /// Refines cost where present; silent where absent.
    Advisory,
}

/// An index into a network's precomputed per-mode cost table.
///
/// Deliberately *not* an enum. The engine needs an index; it does not
/// need to know that index 0 means walking. Naming modes is a profile
/// concern (`turbo-profile-no`) — baking `Foot | Bicycle | Ski` into the
/// model would leak the hiking use case into a general engine and fail
/// the game-engine test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModeId(pub u8);


/// Resolved scalar tuning, keyed by contributor name then parameter.
///
/// This is the mechanism that makes per-request tuning affordable, and
/// the reason it exists is measured rather than assumed. Experiment E4
/// timed rebuilding the cost model: **555 ms on a 1 M-edge graph, 2.8 s
/// at 5 M** — two to eleven times an entire 250 ms solve, because the
/// trail-proximity R-trees are rebuilt from scratch. Rebinding the same
/// stack against new scalars costs **0.098 µs per contributor**: an
/// `Arc` clone and a few scalar writes. The ratio at national scale is
/// roughly 5.6 million to one, so "just rebuild it per request" is not
/// a viable alternative and the split is mandatory.
///
/// Hence the shape of every contributor: an `Arc<Index>` holding the
/// expensive spatial structure, plus plain scalars that
/// [`rebind`](ParamSet) replaces.
///
/// # What this is not
///
/// It is **not** a config file, a preset name, or a patch to merge. The
/// engine receives resolved numbers. Parsing TOML, resolving preset
/// inheritance and applying overrides all happen at the composition
/// layer, which hands down a finished `ParamSet`. If this type ever
/// grows a `from_toml`, the boundary has leaked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamSet {
    entries: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
}

impl ParamSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `contributor.key = value`, replacing any previous binding.
    pub fn set(&mut self, contributor: &str, key: &str, value: f64) -> &mut Self {
        self.entries
            .entry(contributor.to_string())
            .or_default()
            .insert(key.to_string(), value);
        self
    }

    /// The bound value, or `None` — meaning "keep whatever the
    /// contributor was constructed with". Absence is never zero.
    pub fn get(&self, contributor: &str, key: &str) -> Option<f64> {
        self.entries.get(contributor)?.get(key).copied()
    }

    /// `get` as an `f32`, for the many parameters stored that way.
    pub fn get_f32(&self, contributor: &str, key: &str) -> Option<f32> {
        self.get(contributor, key).map(|v| v as f32)
    }

    /// Are there any bindings for this contributor? A contributor with
    /// none can skip rebinding entirely and hand back a shared `Arc`.
    pub fn touches(&self, contributor: &str) -> bool {
        self.entries.get(contributor).is_some_and(|m| !m.is_empty())
    }

    /// The contributor names this set binds anything for, in stable
    /// order. Used to detect keys no contributor claimed.
    pub fn contributors(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|(_, m)| !m.is_empty())
            .map(|(c, _)| c.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.values().all(|m| m.is_empty())
    }

    /// Stable content hash. `BTreeMap` iteration order is what makes it
    /// stable — a `HashMap` here would produce a different fingerprint
    /// per process for identical tuning, silently defeating any cache
    /// keyed on it.
    ///
    /// FNV-1a, hand-rolled rather than pulled in, because this crate's
    /// zero-dependency rule is load-bearing (see the module docs) and
    /// `DefaultHasher` is explicitly not stable across releases.
    pub fn fingerprint(&self) -> u64 {
        let mut h = FNV_OFFSET;
        for (c, params) in &self.entries {
            h = fnv_bytes(h, c.as_bytes());
            for (k, v) in params {
                h = fnv_bytes(h, k.as_bytes());
                h = fnv_bytes(h, &v.to_bits().to_le_bytes());
            }
        }
        h
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over a byte run, seeded with `h` so hashes chain.
pub fn fnv_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Fold an `f64` into a running FNV-1a hash. Used by contributor
/// `fingerprint` implementations so their scalar parameters take part.
#[inline]
pub fn fnv_f64(h: u64, v: f64) -> u64 {
    fnv_bytes(h, &v.to_bits().to_le_bytes())
}

/// Seed a fingerprint from a contributor's stable name.
#[inline]
pub fn fnv_name(name: &str) -> u64 {
    fnv_bytes(FNV_OFFSET, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The portability conformance check, in miniature: a "game engine"
    /// driving a `Heightfield` with nothing but an array in memory.
    ///
    /// If this ever needs a file, a config string, or a projection, the
    /// boundary has leaked. It is a compiling test, not a claim.
    struct ChunkHeights {
        cells: Vec<f32>,
        w: u32,
        h: u32,
        cell_m: f32,
    }

    impl Heightfield for ChunkHeights {
        fn height_at(&self, p: Point) -> Option<f32> {
            let (i, j) = self.index(p)?;
            Some(self.cells[j * self.w as usize + i])
        }
        fn covers(&self, p: Point) -> bool {
            self.index(p).is_some()
        }
        fn slope_aspect_at(&self, _p: Point) -> Option<SlopeAspect> {
            Some(SlopeAspect::default())
        }
        fn extent(&self) -> Extent {
            Extent::new(
                0.0,
                0.0,
                self.w as f64 * self.cell_m as f64,
                self.h as f64 * self.cell_m as f64,
            )
        }
        fn resolution_m(&self) -> f32 {
            self.cell_m
        }
    }

    impl ChunkHeights {
        fn index(&self, p: Point) -> Option<(usize, usize)> {
            if p.x < 0.0 || p.y < 0.0 {
                return None;
            }
            let i = (p.x / self.cell_m as f64) as usize;
            let j = (p.y / self.cell_m as f64) as usize;
            (i < self.w as usize && j < self.h as usize).then_some((i, j))
        }
    }

    #[test]
    fn an_in_memory_array_is_a_heightfield() {
        let f: std::sync::Arc<dyn Heightfield> = std::sync::Arc::new(ChunkHeights {
            cells: (0..64).map(|i| i as f32).collect(),
            w: 8,
            h: 8,
            cell_m: 10.0,
        });
        assert_eq!(f.height_at(Point::new(5.0, 5.0)), Some(0.0));
        assert_eq!(f.height_at(Point::new(15.0, 5.0)), Some(1.0));
        assert!(f.covers(Point::new(79.0, 79.0)));
        assert!(!f.covers(Point::new(81.0, 5.0)));
        assert_eq!(f.height_at(Point::new(81.0, 5.0)), None);
        assert_eq!(f.resolution_m(), 10.0);
        assert_eq!(f.extent(), Extent::new(0.0, 0.0, 80.0, 80.0));
    }

    /// `covers` is authority, `height_at` is value: a hole *inside* the
    /// field is covered but valueless. Collapsing these two is D1.
    #[test]
    fn coverage_and_value_are_independent() {
        struct Holed;
        impl Heightfield for Holed {
            fn height_at(&self, p: Point) -> Option<f32> {
                (p.x != 5.0).then_some(1.0)
            }
            fn covers(&self, _p: Point) -> bool {
                true
            }
            fn slope_aspect_at(&self, _p: Point) -> Option<SlopeAspect> {
                None
            }
            fn extent(&self) -> Extent {
                Extent::new(0.0, 0.0, 10.0, 10.0)
            }
            fn resolution_m(&self) -> f32 {
                1.0
            }
        }
        let hole = Point::new(5.0, 0.0);
        assert!(Holed.covers(hole), "the field is authoritative here");
        assert_eq!(Holed.height_at(hole), None, "...and it has no value here");
    }
}
