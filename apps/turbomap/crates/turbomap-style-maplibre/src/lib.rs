//! Lower a MapLibre Style Spec document onto [`turbomap_core::VectorStyle`].
//!
//! This is the interchange half of the custom-style story: the tileserver
//! authors styles as MapLibre JSON (`/v1/basemap/style.json`), web/Flutter
//! clients hand that to MapLibre GL directly, and the native renderer runs
//! the same document through this crate. One style file, every renderer.
//!
//! ## Supported subset (fail-loud)
//!
//! - layer types `background`, `fill`, `line`, `symbol`
//! - `fill-color`, `line-color`, `line-width` (constant or legacy
//!   `{"stops": [[zoom, px], …]}` — lowered to piecewise-constant zoom-banded
//!   rules), `text-field` (`{prop}`), `text-size`, `text-color`
//! - legacy filters `["==", key, value]` / `["in", key, v1, v2, …]` and the
//!   modern `["==", ["get", key], value]` form
//! - `minzoom` (inclusive) / `maxzoom` (exclusive, per the spec)
//! - colors as `#rgb` / `#rrggbb` / `#rrggbbaa` / `rgb()` / `rgba()`
//!
//! Anything outside the subset is a [`StyleError::Unsupported`] — a loud
//! parse failure beats a silently wrong map.
//!
//! ## Semantics caveat
//!
//! turbomap's engine is *first-match-wins per feature*, while MapLibre paints
//! every matching layer. The two agree only when a style's filters within one
//! source-layer are mutually exclusive — which the house styles guarantee.
//! Line widths are given in CSS px by the spec; turbomap wants tile-extent
//! units, so widths are scaled by `EXTENT_UNITS_PER_PX` (4096/256).

use serde_json::Value as Json;
use turbomap_core::style::{Color, Filter, Paint, Rule, VectorStyle};

/// MVT extent units per CSS pixel at the standard 4096-extent / 256-px tile.
pub const EXTENT_UNITS_PER_PX: f32 = 4096.0 / 256.0;

const DEFAULT_TEXT_SIZE_PX: f32 = 16.0;

#[derive(Debug, thiserror::Error)]
pub enum StyleError {
    #[error("style JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("style layer `{layer}`: {what}")]
    Unsupported { layer: String, what: String },
}

fn unsupported(layer: &str, what: impl Into<String>) -> StyleError {
    StyleError::Unsupported {
        layer: layer.to_string(),
        what: what.into(),
    }
}

/// Parse a MapLibre style document into a [`VectorStyle`].
pub fn parse_style(json: &str) -> Result<VectorStyle, StyleError> {
    let doc: Json = serde_json::from_str(json)?;
    let mut style = VectorStyle::default();

    let layers = doc["layers"].as_array().cloned().unwrap_or_default();
    for layer in &layers {
        let id = layer["id"].as_str().unwrap_or("<unnamed>");
        let ty = layer["type"].as_str().unwrap_or_default();
        match ty {
            "background" => {
                style.background = parse_color(id, &layer["paint"]["background-color"])?;
            }
            "fill" | "line" | "symbol" => {
                style.rules.extend(lower_layer(id, ty, layer)?);
            }
            // turbomap renders relief from its own hillshade pass over the DEM
            // source, not from a style `hillshade` layer; raster layers aren't
            // part of the vector style. Skip rather than reject.
            "hillshade" | "raster" => {}
            other => return Err(unsupported(id, format!("layer type `{other}`"))),
        }
    }
    Ok(style)
}

/// Lower one MapLibre layer into one or more zoom-banded [`Rule`]s.
fn lower_layer(id: &str, ty: &str, layer: &Json) -> Result<Vec<Rule>, StyleError> {
    let source_layer = layer["source-layer"]
        .as_str()
        .ok_or_else(|| unsupported(id, "missing source-layer"))?
        .to_string();
    let filter = parse_filter(id, layer.get("filter"))?;
    let min_zoom = layer["minzoom"].as_u64().unwrap_or(0) as u8;
    // Spec: maxzoom is exclusive; Rule.max_zoom is inclusive.
    let max_zoom = layer["maxzoom"]
        .as_u64()
        .map(|z| (z as u8).saturating_sub(1))
        .unwrap_or(22);
    let paint = &layer["paint"];

    let base = Rule {
        source_layer,
        filter,
        min_zoom,
        max_zoom,
        ..Rule::default()
    };

    match ty {
        "fill" => Ok(vec![Rule {
            paint: Paint::Fill {
                color: parse_color(id, &paint["fill-color"])?,
            },
            ..base
        }]),
        "symbol" => {
            let field = layer["layout"]["text-field"]
                .as_str()
                .ok_or_else(|| unsupported(id, "symbol without layout.text-field"))?;
            let text_field = field
                .strip_prefix('{')
                .and_then(|f| f.strip_suffix('}'))
                .ok_or_else(|| unsupported(id, format!("text-field `{field}` (want `{{prop}}`)")))?
                .to_string();
            let font_size_px = layer["layout"]["text-size"]
                .as_f64()
                .unwrap_or(f64::from(DEFAULT_TEXT_SIZE_PX)) as f32;
            let color = match paint.get("text-color") {
                Some(c) => parse_color(id, c)?,
                None => Color::rgb(0, 0, 0),
            };
            Ok(vec![Rule {
                paint: Paint::Text {
                    text_field,
                    font_size_px,
                    color,
                    // The supported MapLibre subset (see the crate docs) does
                    // not yet lower halo / icon / placement / spacing, so the
                    // new Paint::Text knobs default to no-ops — identical
                    // rendering to before main extended the variant.
                    halo_color: Color::rgba(0, 0, 0, 0),
                    halo_width: 0.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 0.0,
                },
                ..base
            }])
        }
        "line" => {
            let color = parse_color(id, &paint["line-color"])?;
            match &paint["line-width"] {
                Json::Null => Ok(vec![Rule {
                    paint: Paint::Line {
                        color,
                        width: EXTENT_UNITS_PER_PX,
                    },
                    ..base
                }]),
                w if w.is_number() => Ok(vec![Rule {
                    paint: Paint::Line {
                        color,
                        width: w.as_f64().unwrap() as f32 * EXTENT_UNITS_PER_PX,
                    },
                    ..base
                }]),
                Json::Object(o) => {
                    // Legacy zoom function: lowered to piecewise-constant
                    // rules, one per zoom band, so the existing engine's
                    // per-rule zoom range expresses the zoom dependence.
                    let stops = o
                        .get("stops")
                        .and_then(|s| s.as_array())
                        .ok_or_else(|| unsupported(id, "line-width object without stops"))?;
                    let mut bands: Vec<(u8, f32)> = Vec::with_capacity(stops.len());
                    for stop in stops {
                        let z = stop[0]
                            .as_u64()
                            .ok_or_else(|| unsupported(id, "non-integer stop zoom"))?
                            as u8;
                        let w = stop[1]
                            .as_f64()
                            .ok_or_else(|| unsupported(id, "non-numeric stop width"))?
                            as f32;
                        bands.push((z, w));
                    }
                    if bands.is_empty() {
                        return Err(unsupported(id, "empty stops"));
                    }
                    let mut rules = Vec::with_capacity(bands.len());
                    for (i, (z, w)) in bands.iter().enumerate() {
                        let band_min = if i == 0 { base.min_zoom } else { *z };
                        let band_max = match bands.get(i + 1) {
                            Some((next_z, _)) => next_z.saturating_sub(1),
                            None => base.max_zoom,
                        };
                        rules.push(Rule {
                            paint: Paint::Line {
                                color,
                                width: w * EXTENT_UNITS_PER_PX,
                            },
                            min_zoom: band_min,
                            max_zoom: band_max,
                            ..base.clone()
                        });
                    }
                    Ok(rules)
                }
                other => Err(unsupported(id, format!("line-width `{other}`"))),
            }
        }
        _ => unreachable!("caller matched type"),
    }
}

fn parse_filter(id: &str, filter: Option<&Json>) -> Result<Filter, StyleError> {
    let Some(f) = filter else {
        return Ok(Filter::Always);
    };
    let arr = f
        .as_array()
        .ok_or_else(|| unsupported(id, "filter is not an array"))?;
    let op = arr
        .first()
        .and_then(|o| o.as_str())
        .ok_or_else(|| unsupported(id, "filter without operator"))?;
    let key = filter_key(id, arr.get(1))?;
    match op {
        "==" => {
            let val = arr
                .get(2)
                .map(json_to_match_string)
                .ok_or_else(|| unsupported(id, "== without value"))?;
            Ok(Filter::Eq(key, val))
        }
        "in" => {
            let vals: Vec<String> = arr[2..].iter().map(json_to_match_string).collect();
            if vals.is_empty() {
                return Err(unsupported(id, "in without values"));
            }
            Ok(Filter::In(key, vals))
        }
        other => Err(unsupported(id, format!("filter op `{other}`"))),
    }
}

/// Accept both the legacy bare key and the modern `["get", key]` form.
fn filter_key(id: &str, key: Option<&Json>) -> Result<String, StyleError> {
    match key {
        Some(Json::String(s)) => Ok(s.clone()),
        Some(Json::Array(a))
            if a.first().and_then(|o| o.as_str()) == Some("get") && a.get(1).is_some() =>
        {
            a[1].as_str()
                .map(str::to_string)
                .ok_or_else(|| unsupported(id, "get with non-string key"))
        }
        _ => Err(unsupported(id, "filter key")),
    }
}

/// Stringify a filter operand the way `turbomap_core`'s matcher compares:
/// it parses the target string back against the MVT value's native type, so
/// bools become "true"/"false" and numbers their decimal form.
fn json_to_match_string(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn parse_color(id: &str, v: &Json) -> Result<Color, StyleError> {
    let s = v
        .as_str()
        .ok_or_else(|| unsupported(id, format!("color `{v}`")))?;
    color_from_css(s).ok_or_else(|| unsupported(id, format!("color `{s}`")))
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)`.
pub fn color_from_css(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let h = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        return match hex.len() {
            3 => {
                let d = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|v| v * 17);
                Some(Color::rgb(d(0)?, d(1)?, d(2)?))
            }
            6 => Some(Color::rgb(h(0)?, h(2)?, h(4)?)),
            8 => Some(Color::rgba(h(0)?, h(2)?, h(4)?, h(6)?)),
            _ => None,
        };
    }
    let inner = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let chan = |i: usize| parts.get(i)?.parse::<u8>().ok();
    match parts.len() {
        3 => Some(Color::rgb(chan(0)?, chan(1)?, chan(2)?)),
        4 => {
            let a = parts[3].parse::<f32>().ok()?;
            Some(Color::rgba(
                chan(0)?,
                chan(1)?,
                chan(2)?,
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real house style, single source of truth in the tileserver tree.
    /// Parsing it here is the cross-app contract test: if the style grows
    /// past this crate's subset, this fails at `cargo test` time, not on a
    /// user's screen.
    fn n50_topo() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tileserver/styles/n50-topo.json");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    #[test]
    fn parses_the_house_n50_topo_style() {
        let style = parse_style(&n50_topo()).expect("n50-topo parses");
        assert_eq!(style.background, Color::rgb(0xf3, 0xf1, 0xea));
        // Every basemap source layer the style covers must yield rules.
        for sl in [
            "water",
            "glacier",
            "landcover",
            "building",
            "coastline",
            "contour",
            "transportation",
            "place",
        ] {
            assert!(
                style.rules.iter().any(|r| r.source_layer == sl),
                "no rule lowered for source-layer `{sl}`"
            );
        }
    }

    #[test]
    fn index_contours_are_heavier_than_regular() {
        let style = parse_style(&n50_topo()).unwrap();
        let width_of = |target: &str| {
            style
                .rules
                .iter()
                .find_map(|r| match (&r.filter, &r.paint) {
                    (Filter::Eq(k, v), Paint::Line { width, .. })
                        if r.source_layer == "contour" && k == "is_index" && v == target =>
                    {
                        Some(*width)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no contour rule for is_index={target}"))
        };
        assert!(width_of("true") > width_of("false"));
    }

    #[test]
    fn line_width_stops_lower_to_contiguous_zoom_bands() {
        let style = parse_style(&n50_topo()).unwrap();
        // road-vei: stops [[9,0.8],[12,1.6],[15,3.0]] → three bands covering
        // 9..=22 contiguously with strictly increasing widths.
        let mut bands: Vec<&Rule> = style
            .rules
            .iter()
            .filter(|r| {
                r.source_layer == "transportation"
                    && matches!(&r.filter, Filter::Eq(k, v) if k == "class" && v == "vei")
            })
            .collect();
        bands.sort_by_key(|r| r.min_zoom);
        assert_eq!(bands.len(), 3);
        assert_eq!((bands[0].min_zoom, bands[0].max_zoom), (9, 11));
        assert_eq!((bands[1].min_zoom, bands[1].max_zoom), (12, 14));
        assert_eq!((bands[2].min_zoom, bands[2].max_zoom), (15, 22));
        let widths: Vec<f32> = bands
            .iter()
            .map(|r| match r.paint {
                Paint::Line { width, .. } => width,
                _ => unreachable!(),
            })
            .collect();
        assert!(widths[0] < widths[1] && widths[1] < widths[2]);
        // px → extent units conversion applied.
        assert!((widths[0] - 0.8 * EXTENT_UNITS_PER_PX).abs() < 1e-3);
    }

    #[test]
    fn symbol_layers_become_text_rules() {
        let style = parse_style(&n50_topo()).unwrap();
        let summit = style
            .rules
            .iter()
            .find(|r| {
                r.source_layer == "place"
                    && matches!(&r.filter, Filter::Eq(k, v) if k == "kind" && v == "summit")
            })
            .expect("summit symbol rule");
        match &summit.paint {
            Paint::Text {
                text_field,
                font_size_px,
                ..
            } => {
                assert_eq!(text_field, "name");
                assert_eq!(*font_size_px, 11.0);
            }
            other => panic!("expected text paint, got {other:?}"),
        }
        assert_eq!(summit.min_zoom, 10);
    }

    #[test]
    fn css_colors_parse() {
        assert_eq!(color_from_css("#fff"), Some(Color::rgb(255, 255, 255)));
        assert_eq!(
            color_from_css("#a0522d"),
            Some(Color::rgb(0xa0, 0x52, 0x2d))
        );
        assert_eq!(
            color_from_css("#11223344"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0x44))
        );
        assert_eq!(color_from_css("rgb(1, 2, 3)"), Some(Color::rgb(1, 2, 3)));
        assert_eq!(
            color_from_css("rgba(10,20,30,0.5)"),
            Some(Color::rgba(10, 20, 30, 128))
        );
        assert_eq!(color_from_css("magenta"), None);
    }

    #[test]
    fn unsupported_constructs_fail_loud() {
        let bad = r##"{"layers":[{"id":"x","type":"fill-extrusion"}]}"##;
        assert!(matches!(
            parse_style(bad),
            Err(StyleError::Unsupported { .. })
        ));
        let bad_filter = r##"{"layers":[{"id":"x","type":"fill","source-layer":"water",
            "filter":["has","name"],"paint":{"fill-color":"#fff"}}]}"##;
        assert!(parse_style(bad_filter).is_err());
    }

    #[test]
    fn modern_get_filter_form_is_accepted() {
        let s = r##"{"layers":[{"id":"x","type":"fill","source-layer":"water",
            "filter":["==",["get","kind"],"sea"],"paint":{"fill-color":"#fff"}}]}"##;
        let style = parse_style(s).unwrap();
        assert_eq!(
            style.rules[0].filter,
            Filter::Eq("kind".into(), "sea".into())
        );
    }

    #[test]
    fn boolean_filter_values_stringify_for_the_engine() {
        // The engine's value_eq_str parses "true" back against Value::Bool.
        let s = r##"{"layers":[{"id":"x","type":"line","source-layer":"contour",
            "filter":["==","is_index",true],"paint":{"line-color":"#fff","line-width":1}}]}"##;
        let style = parse_style(s).unwrap();
        assert_eq!(
            style.rules[0].filter,
            Filter::Eq("is_index".into(), "true".into())
        );
    }
}
