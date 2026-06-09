//! Paint values, zoom curves, colours, and feature filters.
//!
//! A `Paint<T>` is the unit of styling. Phase 1 supports two of the three
//! planned forms — `Const` and `Zoom` (a zoom-interpolated curve). The
//! third, `Data` (data-driven expressions evaluated on the GPU), lands in
//! Phase 3; the enum is intentionally non-exhaustive-friendly so adding it
//! is backward compatible.

use serde::{Deserialize, Serialize};

/// 8-bit RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// Linear interpolation between two style values. Used to evaluate a
/// [`Paint::Zoom`] curve between its stops.
pub trait Interpolate: Copy {
    fn interpolate(a: Self, b: Self, t: f64) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(a: Self, b: Self, t: f64) -> Self {
        a + (b - a) * t as f32
    }
}

impl Interpolate for f64 {
    fn interpolate(a: Self, b: Self, t: f64) -> Self {
        a + (b - a) * t
    }
}

impl Interpolate for Color {
    fn interpolate(a: Self, b: Self, t: f64) -> Self {
        let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round().clamp(0.0, 255.0) as u8;
        Color {
            r: lerp(a.r, b.r),
            g: lerp(a.g, b.g),
            b: lerp(a.b, b.b),
            a: lerp(a.a, b.a),
        }
    }
}

/// One control point of a zoom curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoomStop<T> {
    pub zoom: f64,
    pub value: T,
}

/// A styling value, possibly varying with zoom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Paint<T> {
    /// A single value at every zoom.
    Const(T),
    /// A piecewise-linear curve over zoom. Stops must be non-empty and
    /// sorted ascending by `zoom`; values clamp outside the stop range.
    Zoom { stops: Vec<ZoomStop<T>> },
}

impl<T: Interpolate> Paint<T> {
    /// Resolve the paint at a given zoom.
    pub fn at(&self, zoom: f64) -> T {
        match self {
            Paint::Const(v) => *v,
            Paint::Zoom { stops } => {
                assert!(!stops.is_empty(), "Paint::Zoom requires at least one stop");
                if zoom <= stops[0].zoom {
                    return stops[0].value;
                }
                let last = &stops[stops.len() - 1];
                if zoom >= last.zoom {
                    return last.value;
                }
                // Find the bracketing pair. Stops are assumed sorted.
                for pair in stops.windows(2) {
                    let (lo, hi) = (&pair[0], &pair[1]);
                    if zoom >= lo.zoom && zoom <= hi.zoom {
                        let span = hi.zoom - lo.zoom;
                        let t = if span <= f64::EPSILON {
                            0.0
                        } else {
                            (zoom - lo.zoom) / span
                        };
                        return T::interpolate(lo.value, hi.value, t);
                    }
                }
                last.value
            }
        }
    }
}

impl<T> Paint<T> {
    /// Convenience constructor for a constant paint.
    pub fn constant(value: T) -> Self {
        Paint::Const(value)
    }
}

/// A scalar a [`Filter`] can compare against a feature property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    Bool(bool),
    Number(f64),
    String(String),
}

/// Predicate selecting which features a vector layer styles. A strict
/// superset of the renderer's current `Eq`/`In` matcher.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filter {
    /// Matches every feature.
    #[default]
    Always,
    /// Property equals a value.
    Eq(String, FilterValue),
    /// Property is one of a set.
    In(String, Vec<FilterValue>),
    /// Logical negation.
    Not(Box<Filter>),
    /// All sub-filters match.
    All(Vec<Filter>),
    /// Any sub-filter matches.
    Any(Vec<Filter>),
}
