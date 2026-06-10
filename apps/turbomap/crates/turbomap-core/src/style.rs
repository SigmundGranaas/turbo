//! A minimal vector-tile style. Intentionally narrower than the Mapbox
//! Style Spec — just enough for an opinionated demo, with room to grow.
//!
//! A `VectorStyle` is an ordered list of `Rule`s. The tessellator walks each
//! feature, finds the *first* matching rule (rules are tried in declaration
//! order), and emits geometry in that rule's paint. Features that don't
//! match any rule are skipped.

use crate::dem::DemEncoding;
use crate::vector::{Feature, GeomType, Value};

/// 8-bit sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    /// The colour-management contract: colours are *authored* in sRGB
    /// (what design tools and style specs produce), but every render
    /// target is `*Srgb`, meaning the GPU re-encodes on write and blends
    /// in linear space. Anything fed to a shader as a colour therefore
    /// has to be decoded sRGB→linear exactly once, here. Skipping this
    /// double-encodes and visibly washes out everything darker than
    /// white (the bug the simulator's colour histogram caught).
    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            srgb_channel_to_linear(self.r),
            srgb_channel_to_linear(self.g),
            srgb_channel_to_linear(self.b),
            self.a as f32 / 255.0, // alpha is coverage — never gamma-encoded
        ]
    }

    /// [`Self::to_linear_f32`] quantised to bytes, for `Unorm8x4` vertex
    /// attributes. Flat fills don't band; if gradients ever do, the
    /// decode moves into the shader instead.
    pub fn to_linear_bytes(self) -> [u8; 4] {
        let [r, g, b, a] = self.to_linear_f32();
        let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        [q(r), q(g), q(b), q(a)]
    }
}

/// Exact (piecewise) sRGB EOTF for one 8-bit channel.
fn srgb_channel_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// What to do with a matching feature.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Fill {
        color: Color,
    },
    /// `width` is in tile-local extent units (typical extent is 4096). 30
    /// units at extent 4096 ≈ a 2-pixel road at z=14.
    Line {
        color: Color,
        width: f32,
    },
    /// Render the feature's value at `text_field` as a text label at the
    /// feature's point. `font_size_px` is the rasterised line height.
    ///
    /// The label is drawn from the SDF atlas with an optional halo (the
    /// readability outline real maps use over busy ground). `halo_width`
    /// is in glyph pixels (0 = no halo); `halo_color` its colour.
    Text {
        text_field: String,
        font_size_px: f32,
        color: Color,
        halo_color: Color,
        halo_width: f32,
        /// Numeric feature property ranking placement importance (higher
        /// wins collisions). `None` falls back to font size, so larger
        /// labels still beat smaller ones.
        rank_field: Option<String>,
    },
}

/// Match a feature property by exact equality. `None` matches any feature
/// regardless of properties.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    /// Match every feature in the layer.
    Always,
    /// Match if property `key` equals the given value (string compare).
    Eq(String, String),
    /// Match if property `key` is in any of the given values.
    In(String, Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Rule {
    /// The MVT source layer this rule applies to (e.g. `"water"`,
    /// `"transportation"`).
    pub source_layer: String,
    pub filter: Filter,
    pub paint: Paint,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// When `true`, features matching this rule are retained in the cache
    /// so they can be picked up by `VectorMap::hit_test`. Off by default —
    /// decorative layers (water, background) don't need to be clickable.
    pub interactive: bool,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            source_layer: String::new(),
            filter: Filter::Always,
            paint: Paint::Fill {
                color: Color::default(),
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VectorStyle {
    /// Background colour, painted before any vector geometry.
    pub background: Color,
    pub rules: Vec<Rule>,
}

/// Configuration for a hillshade layer. The DEM source's tiles are
/// regular raster PNG/WebP, but each pixel encodes an elevation; the
/// pipeline decodes per fragment and derives lighting from the gradient.
#[derive(Debug, Clone, Copy)]
pub struct HillshadeStyle {
    /// How tile RGB values map to metres. Plug in `MapboxRgb` for
    /// Maptiler/Mapbox-style DEM tiles, `Terrarium` for AWS-style.
    pub encoding: DemEncoding,
    /// Sun azimuth in degrees, measured clockwise from north (0 = N, 90
    /// = E). Outdoor maps conventionally use 315° (NW).
    pub sun_azimuth_deg: f32,
    /// Sun altitude in degrees above the horizon (90 = noon overhead).
    pub sun_altitude_deg: f32,
    /// Vertical exaggeration multiplier on the decoded elevation. 1.0 =
    /// true to the encoding; larger values make slopes visually steeper.
    pub exaggeration: f32,
    /// Colour applied to deep-shadow fragments (intensity ≈ 0).
    pub shadow_color: Color,
    /// Colour applied to fully-lit fragments (intensity ≈ 1).
    pub highlight_color: Color,
    /// Final alpha multiplier — `1.0` opaque, lower lets underlying
    /// layers show through.
    pub opacity: f32,
}

impl Default for HillshadeStyle {
    fn default() -> Self {
        Self {
            encoding: DemEncoding::MapboxRgb,
            sun_azimuth_deg: 315.0,
            sun_altitude_deg: 45.0,
            exaggeration: 1.0,
            shadow_color: Color::rgb(60, 50, 40),
            highlight_color: Color::rgb(255, 248, 235),
            opacity: 0.55,
        }
    }
}

impl VectorStyle {
    /// Returns the index of the first matching rule for `feature` from
    /// `source_layer` at `zoom`, or `None` if no rule matches.
    pub fn matching_rule(&self, source_layer: &str, feature: &Feature, zoom: u8) -> Option<usize> {
        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.source_layer != source_layer {
                continue;
            }
            if zoom < rule.min_zoom || zoom > rule.max_zoom {
                continue;
            }
            if !filter_matches(&rule.filter, feature) {
                continue;
            }
            if !paint_matches_geom_type(&rule.paint, feature.geom_type) {
                continue;
            }
            return Some(idx);
        }
        None
    }
}

fn filter_matches(filter: &Filter, feature: &Feature) -> bool {
    match filter {
        Filter::Always => true,
        Filter::Eq(k, v) => feature
            .properties
            .get(k)
            .map(|val| value_eq_str(val, v))
            .unwrap_or(false),
        Filter::In(k, vs) => feature
            .properties
            .get(k)
            .map(|val| vs.iter().any(|v| value_eq_str(val, v)))
            .unwrap_or(false),
    }
}

fn paint_matches_geom_type(paint: &Paint, gt: GeomType) -> bool {
    match (paint, gt) {
        (Paint::Fill { .. }, GeomType::Polygon) => true,
        (Paint::Line { .. }, GeomType::LineString) => true,
        (Paint::Text { .. }, GeomType::Point) => true,
        // Outline-as-line for polygons would be possible but skip for now.
        _ => false,
    }
}

fn value_eq_str(v: &Value, target: &str) -> bool {
    match v {
        Value::String(s) => s == target,
        Value::Int(i) => target.parse::<i64>().map(|t| t == *i).unwrap_or(false),
        Value::UInt(u) => target.parse::<u64>().map(|t| t == *u).unwrap_or(false),
        Value::Float(f) => target.parse::<f64>().map(|t| t == *f).unwrap_or(false),
        Value::Bool(b) => target.parse::<bool>().map(|t| t == *b).unwrap_or(false),
        Value::Null => false,
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a developer builds a style and expects exactly the
    //! features they describe to be drawn — with the right paint, at the
    //! right zooms, filtered by property. Getting any of these wrong
    //! silently is the worst kind of style bug; the tests pin the
    //! semantics.

    use super::*;
    use crate::vector::{Feature, GeomType, Geometry, Value};
    use std::collections::HashMap;

    fn poly_feature(props: &[(&str, &str)]) -> Feature {
        let mut p = HashMap::new();
        for (k, v) in props {
            p.insert((*k).to_owned(), Value::String((*v).to_owned()));
        }
        Feature {
            id: 0,
            geom_type: GeomType::Polygon,
            geometry: Geometry::Polygon(vec![]),
            properties: p,
        }
    }

    fn line_feature(props: &[(&str, &str)]) -> Feature {
        let mut p = HashMap::new();
        for (k, v) in props {
            p.insert((*k).to_owned(), Value::String((*v).to_owned()));
        }
        Feature {
            id: 0,
            geom_type: GeomType::LineString,
            geometry: Geometry::LineString(vec![]),
            properties: p,
        }
    }

    #[test]
    fn rules_pick_features_by_source_layer_name() {
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "water".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(0, 0, 255),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let f = poly_feature(&[]);
        assert_eq!(style.matching_rule("water", &f, 10), Some(0));
        assert_eq!(style.matching_rule("buildings", &f, 10), None);
    }

    #[test]
    fn property_filter_eq_matches_only_the_right_value() {
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "transportation".into(),
                filter: Filter::Eq("class".into(), "motorway".into()),
                paint: Paint::Line {
                    color: Color::rgb(255, 0, 0),
                    width: 30.0,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        assert_eq!(
            style.matching_rule(
                "transportation",
                &line_feature(&[("class", "motorway")]),
                10
            ),
            Some(0)
        );
        assert_eq!(
            style.matching_rule(
                "transportation",
                &line_feature(&[("class", "residential")]),
                10
            ),
            None
        );
        assert_eq!(
            style.matching_rule("transportation", &line_feature(&[]), 10),
            None,
            "missing property must not match",
        );
    }

    #[test]
    fn filter_in_matches_any_value_in_the_set() {
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "transportation".into(),
                filter: Filter::In(
                    "class".into(),
                    vec!["motorway".into(), "trunk".into(), "primary".into()],
                ),
                paint: Paint::Line {
                    color: Color::rgb(255, 0, 0),
                    width: 20.0,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        assert_eq!(
            style.matching_rule(
                "transportation",
                &line_feature(&[("class", "motorway")]),
                10
            ),
            Some(0)
        );
        assert_eq!(
            style.matching_rule("transportation", &line_feature(&[("class", "primary")]), 10),
            Some(0)
        );
        assert_eq!(
            style.matching_rule("transportation", &line_feature(&[("class", "service")]), 10),
            None
        );
    }

    #[test]
    fn zoom_range_keeps_features_inside_the_inclusive_window() {
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "buildings".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(200, 200, 200),
                },
                min_zoom: 14,
                max_zoom: 18,
                interactive: false,
            }],
        };
        let f = poly_feature(&[]);
        assert_eq!(style.matching_rule("buildings", &f, 13), None);
        assert_eq!(style.matching_rule("buildings", &f, 14), Some(0));
        assert_eq!(style.matching_rule("buildings", &f, 18), Some(0));
        assert_eq!(style.matching_rule("buildings", &f, 19), None);
    }

    #[test]
    fn earlier_rules_win_over_later_rules_for_the_same_feature() {
        // Useful when an overlay style needs a "specific" rule before a
        // "catch-all" rule (e.g. motorways red, all roads grey).
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![
                Rule {
                    source_layer: "transportation".into(),
                    filter: Filter::Eq("class".into(), "motorway".into()),
                    paint: Paint::Line {
                        color: Color::rgb(255, 0, 0),
                        width: 30.0,
                    },
                    min_zoom: 0,
                    max_zoom: 22,
                    interactive: false,
                },
                Rule {
                    source_layer: "transportation".into(),
                    filter: Filter::Always,
                    paint: Paint::Line {
                        color: Color::rgb(180, 180, 180),
                        width: 10.0,
                    },
                    min_zoom: 0,
                    max_zoom: 22,
                    interactive: false,
                },
            ],
        };
        assert_eq!(
            style.matching_rule(
                "transportation",
                &line_feature(&[("class", "motorway")]),
                10
            ),
            Some(0)
        );
        assert_eq!(
            style.matching_rule(
                "transportation",
                &line_feature(&[("class", "residential")]),
                10
            ),
            Some(1)
        );
    }

    #[test]
    fn paint_kind_must_match_geometry_kind() {
        // A Fill rule should not pick up a LineString feature even if every
        // other check passes — silently filling lines causes broken-looking
        // maps that are hard to debug later.
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "boundary".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(0, 0, 0),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let line = line_feature(&[]);
        assert_eq!(style.matching_rule("boundary", &line, 10), None);
    }
}
