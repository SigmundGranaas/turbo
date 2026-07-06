//! Lower a MapLibre Style Spec document onto **Scene IR layers** (plan
//! P6.2) instead of a core `VectorStyle`.
//!
//! This is the authoring-surface half of the adoption plan: the engine
//! already compiles IR `fill`/`line`/`symbol`/`fill-extrusion` layers into
//! the core `VectorStyle` (`turbomap_engine::engine::compile_vector_layer_style`),
//! so a host that consumes MapLibre JSON should produce IR layers and let
//! the engine do the lowering — one compile path, not two.
//!
//! ## Supported subset (fail-loud)
//!
//! Everything [`crate::parse_style`] supports, mapped to the IR:
//!
//! - layer types `fill`, `line`, `symbol`, plus `fill-extrusion`
//!   (`fill-extrusion-color`, constant `fill-extrusion-height` or
//!   `["get", prop]`, `fill-extrusion-base` as `["get", prop]`)
//! - `fill-color` / `line-color` / `text-color` as a constant, a legacy
//!   `{"stops": …}` zoom function, `["interpolate", ["linear"], ["zoom"], …]`
//!   (both → [`Paint::Zoom`]), or `["match", ["get", key], …]`
//!   (→ [`Paint::Match`])
//! - `line-width` constant, legacy stops, or linear interpolate. Where
//!   `parse_style` lowers stops to piecewise-constant zoom-banded rules,
//!   the IR carries the curve itself and the engine interpolates
//!   **linearly between stops** — MapLibre's actual semantics; the two
//!   paths agree exactly at the stop zooms
//! - filters `==`, `!=`, `in`, `!in` (legacy key or `["get", key]`),
//!   `all`, `any`
//! - layer `minzoom` (inclusive) / `maxzoom` (exclusive, per the spec) —
//!   composed into the layer filter as [`Filter::ZoomRange`], which the
//!   engine lowers onto each compiled rule's zoom band
//! - `background` via [`parse_style_background`] (the Scene has no
//!   background layer; the host decides what to do with it)
//! - `hillshade` / `raster` style layers are skipped, exactly as in
//!   `parse_style`: turbomap's terrain/basemap are scene-level concerns
//!
//! ## Documented gaps (dropped, not approximated)
//!
//! - `icon-image` names a style sprite; the IR's `icon_image` names the
//!   renderer's built-in atlas. No mapping exists, so icons are dropped —
//!   the same behaviour as `parse_style`.
//! - `text-offset` / non-centre `text-anchor` / `text-font` /
//!   `line-dasharray` are ignored, as in `parse_style`.
//!
//! ## Semantics note
//!
//! `parse_style` folds every style layer into ONE `VectorStyle`, where
//! rules are first-match-wins per feature. Here each style layer becomes
//! its own IR layer, and every matching layer draws — which is MapLibre's
//! real model. The two agree when filters within one source-layer are
//! mutually exclusive, which the house styles guarantee (and which the
//! fidelity tests check by probing features through both paths).

use serde_json::Value as Json;
use turbomap_scene::style::ZoomStop;
use turbomap_scene::{
    Color, Filter, FilterValue, Layer, MatchCase, Paint, SymbolPlacement, TextAnchor,
};

use crate::{
    color_from_css, filter_key, unsupported, StyleError, DEFAULT_TEXT_SIZE_PX, EXTENT_UNITS_PER_PX,
};

/// Parse a MapLibre style document into Scene IR layers drawing from the
/// vector source named `source`. Order is preserved (bottom-to-top).
pub fn parse_style_layers(json: &str, source: &str) -> Result<Vec<Layer>, StyleError> {
    let doc: Json = serde_json::from_str(json)?;
    let mut out = Vec::new();
    for layer in doc["layers"].as_array().cloned().unwrap_or_default() {
        let id = layer["id"].as_str().unwrap_or("<unnamed>");
        let ty = layer["type"].as_str().unwrap_or_default();
        match ty {
            // Background is not a feature layer — surface it through
            // `parse_style_background`. Hillshade/raster stay scene-level
            // concerns (terrain source / raster layers), as in parse_style.
            "background" | "hillshade" | "raster" => {}
            "fill" | "line" | "symbol" | "fill-extrusion" => {
                out.push(lower_ir_layer(id, ty, &layer, source)?);
            }
            other => return Err(unsupported(id, format!("layer type `{other}`"))),
        }
    }
    Ok(out)
}

/// The style's `background` layer colour, if it has one. The Scene IR has
/// no background layer — a host over a raster bed usually wants none, and
/// a pure-vector host paints it behind the stack — so it is surfaced
/// separately rather than silently dropped.
pub fn parse_style_background(json: &str) -> Result<Option<Color>, StyleError> {
    let doc: Json = serde_json::from_str(json)?;
    for layer in doc["layers"].as_array().into_iter().flatten() {
        if layer["type"].as_str() == Some("background") {
            let id = layer["id"].as_str().unwrap_or("<unnamed>");
            return Ok(Some(parse_ir_color(
                id,
                &layer["paint"]["background-color"],
            )?));
        }
    }
    Ok(None)
}

/// Strip the water-body **fill** layers (source layers `water` / `ocean` /
/// `water_polygons` — the OMT + VersaTiles conventions): the IR-level
/// counterpart of `VectorStyle::without_water_fills`, for hosts that layer
/// the vector style over a raster basemap already showing the water.
/// Waterway lines, outlines and labels on those source layers are kept.
#[must_use]
pub fn without_water_fill_layers(layers: Vec<Layer>) -> Vec<Layer> {
    layers
        .into_iter()
        .filter(|l| {
            !matches!(
                l,
                Layer::Fill { source_layer: Some(sl), .. }
                    if matches!(sl.as_str(), "water" | "ocean" | "water_polygons")
            )
        })
        .collect()
}

/// Lower one MapLibre layer into one IR layer.
fn lower_ir_layer(id: &str, ty: &str, layer: &Json, source: &str) -> Result<Layer, StyleError> {
    let source_layer = layer["source-layer"]
        .as_str()
        .ok_or_else(|| unsupported(id, "missing source-layer"))?
        .to_string();
    let min_zoom = layer["minzoom"].as_u64().unwrap_or(0) as u8;
    // Spec: maxzoom is exclusive; the IR zoom window is inclusive.
    let max_zoom = layer["maxzoom"]
        .as_u64()
        .map(|z| (z as u8).saturating_sub(1))
        .unwrap_or(22);
    let filter = parse_ir_filter(id, layer.get("filter"))?.within_zoom(min_zoom, max_zoom);
    let paint = &layer["paint"];

    match ty {
        "fill" => Ok(Layer::Fill {
            id: id.to_string(),
            source: source.to_string(),
            source_layer: Some(source_layer),
            filter,
            color: parse_color_paint(id, &paint["fill-color"])?,
            opacity: Paint::Const(1.0),
        }),
        "line" => Ok(Layer::Line {
            id: id.to_string(),
            source: source.to_string(),
            source_layer: Some(source_layer),
            filter,
            color: parse_color_paint(id, &paint["line-color"])?,
            width: parse_line_width(id, &paint["line-width"])?,
            // MapLibre `line-dasharray` is in line-width units; the IR's
            // dash_array is screen px. parse_style ignored it — parity kept.
            dash_array: None,
        }),
        "fill-extrusion" => {
            let (height_m, height_property) = match &paint["fill-extrusion-height"] {
                Json::Null => (Paint::Const(0.0), None),
                n if n.is_number() => (Paint::Const(n.as_f64().unwrap() as f32), None),
                v => (
                    Paint::Const(0.0),
                    Some(parse_get_property(id, v, "fill-extrusion-height")?),
                ),
            };
            let min_height_property = match &paint["fill-extrusion-base"] {
                Json::Null => None,
                n if n.as_f64() == Some(0.0) => None,
                v => Some(parse_get_property(id, v, "fill-extrusion-base")?),
            };
            Ok(Layer::FillExtrusion {
                id: id.to_string(),
                source: source.to_string(),
                source_layer: Some(source_layer),
                filter,
                color: parse_color_paint(id, &paint["fill-extrusion-color"])?,
                height_m,
                height_property,
                min_height_property,
            })
        }
        "symbol" => {
            let layout = &layer["layout"];
            let field = layout["text-field"]
                .as_str()
                .ok_or_else(|| unsupported(id, "symbol without layout.text-field"))?;
            let text_field = field
                .strip_prefix('{')
                .and_then(|f| f.strip_suffix('}'))
                .ok_or_else(|| unsupported(id, format!("text-field `{field}` (want `{{prop}}`)")))?
                .to_string();
            let text_size = match &layout["text-size"] {
                Json::Null => Paint::Const(DEFAULT_TEXT_SIZE_PX),
                n if n.is_number() => Paint::Const(n.as_f64().unwrap() as f32),
                other => return Err(unsupported(id, format!("text-size `{other}`"))),
            };
            let color = match paint.get("text-color") {
                Some(c) => parse_color_paint(id, c)?,
                None => Paint::Const(Color::rgb(0, 0, 0)),
            };
            let halo_color = match paint.get("text-halo-color") {
                Some(c) => parse_color_paint(id, c)?,
                None => Paint::Const(Color::rgba(0, 0, 0, 0)),
            };
            let halo_width = match paint.get("text-halo-width") {
                None => Paint::Const(0.0),
                Some(w) if w.is_number() => Paint::Const(w.as_f64().unwrap() as f32),
                Some(other) => return Err(unsupported(id, format!("text-halo-width `{other}`"))),
            };
            let placement = match layout["symbol-placement"].as_str() {
                None | Some("point") => SymbolPlacement::Point,
                Some("line") | Some("line-center") => SymbolPlacement::Line,
                Some(other) => return Err(unsupported(id, format!("symbol-placement `{other}`"))),
            };
            Ok(Layer::Symbol {
                id: id.to_string(),
                source: source.to_string(),
                source_layer: Some(source_layer),
                filter,
                text_field,
                text_size,
                color,
                halo_color,
                halo_width,
                sort_key: None,
                placement,
                // MapLibre `icon-image` names a style sprite; the IR's
                // icon_image names the renderer's built-in atlas. No mapping
                // exists, so icons are dropped — the parse_style behaviour.
                icon_image: None,
                icon_size: Paint::Const(24.0),
                icon_color: Paint::Const(Color::rgb(70, 78, 92)),
                text_anchor: TextAnchor::Center,
                letter_spacing: 0.0,
                font_weight: 0.0,
            })
        }
        _ => unreachable!("caller matched type"),
    }
}

/// `["get", key]` → `key`; anything else is unsupported for `what`.
fn parse_get_property(id: &str, v: &Json, what: &str) -> Result<String, StyleError> {
    match v {
        Json::Array(a) if a.first().and_then(|o| o.as_str()) == Some("get") && a.len() == 2 => a[1]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| unsupported(id, format!("{what} get with non-string key"))),
        other => Err(unsupported(id, format!("{what} `{other}`"))),
    }
}

/// A colour paint: constant, legacy `{stops}`, linear zoom interpolate, or
/// a data-driven match on a feature property.
fn parse_color_paint(id: &str, v: &Json) -> Result<Paint<Color>, StyleError> {
    match v {
        Json::String(_) => Ok(Paint::Const(parse_ir_color(id, v)?)),
        Json::Object(o) => Ok(Paint::Zoom {
            stops: parse_legacy_stops(id, o, |s| parse_ir_color(id, s))?,
        }),
        Json::Array(a) => match a.first().and_then(|op| op.as_str()) {
            Some("interpolate") => Ok(Paint::Zoom {
                stops: parse_interpolate(id, a, |s| parse_ir_color(id, s))?,
            }),
            Some("match") => parse_match_paint(id, a, |s| parse_ir_color(id, s)),
            _ => Err(unsupported(id, format!("color expression `{v}`"))),
        },
        other => Err(unsupported(id, format!("color `{other}`"))),
    }
}

/// `line-width`: constant px, legacy stops, or linear zoom interpolate.
/// Widths convert px → the core's units via [`EXTENT_UNITS_PER_PX`], the
/// same conversion `parse_style` bakes into its rules.
fn parse_line_width(id: &str, v: &Json) -> Result<Paint<f32>, StyleError> {
    let px = |w: f64| w as f32 * EXTENT_UNITS_PER_PX;
    let stop_width = |s: &Json| -> Result<f32, StyleError> {
        s.as_f64()
            .map(px)
            .ok_or_else(|| unsupported(id, "non-numeric stop width"))
    };
    match v {
        Json::Null => Ok(Paint::Const(px(1.0))),
        w if w.is_number() => Ok(Paint::Const(px(w.as_f64().unwrap()))),
        Json::Object(o) => Ok(Paint::Zoom {
            stops: parse_legacy_stops(id, o, stop_width)?,
        }),
        Json::Array(a) if a.first().and_then(|op| op.as_str()) == Some("interpolate") => {
            Ok(Paint::Zoom {
                stops: parse_interpolate(id, a, stop_width)?,
            })
        }
        other => Err(unsupported(id, format!("line-width `{other}`"))),
    }
}

/// Legacy zoom function: `{"stops": [[zoom, value], …]}` → zoom stops.
fn parse_legacy_stops<T>(
    id: &str,
    o: &serde_json::Map<String, Json>,
    parse: impl Fn(&Json) -> Result<T, StyleError>,
) -> Result<Vec<ZoomStop<T>>, StyleError> {
    let stops = o
        .get("stops")
        .and_then(|s| s.as_array())
        .ok_or_else(|| unsupported(id, "zoom function without stops"))?;
    let mut out = Vec::with_capacity(stops.len());
    for stop in stops {
        let zoom = stop[0]
            .as_f64()
            .ok_or_else(|| unsupported(id, "non-numeric stop zoom"))?;
        out.push(ZoomStop {
            zoom,
            value: parse(&stop[1])?,
        });
    }
    if out.is_empty() {
        return Err(unsupported(id, "empty stops"));
    }
    Ok(out)
}

/// Modern `["interpolate", ["linear"], ["zoom"], in, out, …]` → zoom stops.
/// Only linear zoom interpolation lowers ([`Paint::Zoom`] is linear);
/// exponential/bezier bases fail loud.
fn parse_interpolate<T>(
    id: &str,
    a: &[Json],
    parse: impl Fn(&Json) -> Result<T, StyleError>,
) -> Result<Vec<ZoomStop<T>>, StyleError> {
    if a.get(1) != Some(&serde_json::json!(["linear"])) {
        return Err(unsupported(id, "interpolate type (only [\"linear\"])"));
    }
    if a.get(2) != Some(&serde_json::json!(["zoom"])) {
        return Err(unsupported(id, "interpolate input (only [\"zoom\"])"));
    }
    let rest = &a[3..];
    if rest.len() < 2 || !rest.len().is_multiple_of(2) {
        return Err(unsupported(id, "interpolate stop pairs"));
    }
    let mut out = Vec::with_capacity(rest.len() / 2);
    for pair in rest.chunks(2) {
        let zoom = pair[0]
            .as_f64()
            .ok_or_else(|| unsupported(id, "non-numeric interpolate zoom"))?;
        out.push(ZoomStop {
            zoom,
            value: parse(&pair[1])?,
        });
    }
    Ok(out)
}

/// `["match", ["get", key], label(s), output, …, fallback]` →
/// [`Paint::Match`]. Array labels expand to one case per value.
fn parse_match_paint<T: Clone>(
    id: &str,
    a: &[Json],
    parse: impl Fn(&Json) -> Result<T, StyleError>,
) -> Result<Paint<T>, StyleError> {
    let property = match a.get(1) {
        Some(v) => parse_get_property(id, v, "match input")?,
        None => return Err(unsupported(id, "match without input")),
    };
    let body = &a[2..];
    if body.len() < 3 || body.len().is_multiple_of(2) {
        return Err(unsupported(
            id,
            "match arity (want label/output pairs + fallback)",
        ));
    }
    let mut cases = Vec::new();
    for pair in body[..body.len() - 1].chunks(2) {
        let result = parse(&pair[1])?;
        match &pair[0] {
            Json::Array(labels) => {
                for l in labels {
                    cases.push(MatchCase {
                        value: json_filter_value(id, l)?,
                        result: result.clone(),
                    });
                }
            }
            scalar => cases.push(MatchCase {
                value: json_filter_value(id, scalar)?,
                result,
            }),
        }
    }
    let default = parse(body.last().expect("checked non-empty"))?;
    Ok(Paint::Match {
        property,
        cases,
        default: Box::new(default),
    })
}

/// Filters: legacy `==`/`!=`/`in`/`!in` (bare key or `["get", key]`) plus
/// the `all`/`any` combinators, with typed operands ([`FilterValue`]) —
/// the engine stringifies them exactly as `parse_style` did.
fn parse_ir_filter(id: &str, filter: Option<&Json>) -> Result<Filter, StyleError> {
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
    match op {
        "all" | "any" => {
            let subs = arr[1..]
                .iter()
                .map(|s| parse_ir_filter(id, Some(s)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(if op == "all" {
                Filter::All(subs)
            } else {
                Filter::Any(subs)
            })
        }
        "==" | "!=" => {
            let key = filter_key(id, arr.get(1))?;
            let val = arr
                .get(2)
                .map(|v| json_filter_value(id, v))
                .transpose()?
                .ok_or_else(|| unsupported(id, format!("{op} without value")))?;
            let eq = Filter::Eq(key, val);
            Ok(if op == "==" {
                eq
            } else {
                Filter::Not(Box::new(eq))
            })
        }
        "in" | "!in" => {
            let key = filter_key(id, arr.get(1))?;
            let vals = arr[2..]
                .iter()
                .map(|v| json_filter_value(id, v))
                .collect::<Result<Vec<_>, _>>()?;
            if vals.is_empty() {
                return Err(unsupported(id, format!("{op} without values")));
            }
            let contains = Filter::In(key, vals);
            Ok(if op == "in" {
                contains
            } else {
                Filter::Not(Box::new(contains))
            })
        }
        other => Err(unsupported(id, format!("filter op `{other}`"))),
    }
}

/// A typed filter/match operand. The engine stringifies these for core's
/// matcher exactly the way `parse_style` stringified JSON scalars.
fn json_filter_value(id: &str, v: &Json) -> Result<FilterValue, StyleError> {
    match v {
        Json::String(s) => Ok(FilterValue::String(s.clone())),
        Json::Bool(b) => Ok(FilterValue::Bool(*b)),
        Json::Number(n) => n
            .as_f64()
            .map(FilterValue::Number)
            .ok_or_else(|| unsupported(id, format!("non-finite number `{n}`"))),
        other => Err(unsupported(id, format!("literal `{other}`"))),
    }
}

/// A CSS colour string → IR colour (same accepted forms as `parse_style`).
fn parse_ir_color(id: &str, v: &Json) -> Result<Color, StyleError> {
    let s = v
        .as_str()
        .ok_or_else(|| unsupported(id, format!("color `{v}`")))?;
    let c = color_from_css(s).ok_or_else(|| unsupported(id, format!("color `{s}`")))?;
    Ok(Color::rgba(c.r, c.g, c.b, c.a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n50_topo() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tileserver/styles/n50-topo.json");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    #[test]
    fn n50_topo_lowers_to_ir_layers_with_zoom_windows() {
        let layers = parse_style_layers(&n50_topo(), "n50").expect("n50-topo lowers to IR");
        // Every non-background/hillshade style layer becomes one IR layer.
        assert_eq!(layers.len(), 14);
        // All draw from the caller's source and carry their source-layer.
        for l in &layers {
            assert_eq!(l.source(), Some("n50"));
        }
        // minzoom composes into the filter as a ZoomRange.
        let building = layers.iter().find(|l| l.id() == "building").unwrap();
        match building {
            Layer::Fill {
                filter,
                source_layer,
                ..
            } => {
                assert_eq!(source_layer.as_deref(), Some("building"));
                assert_eq!(filter, &Filter::ZoomRange { min: 14, max: 22 });
            }
            other => panic!("expected fill, got {other:?}"),
        }
        // A filtered + windowed layer keeps both, range first in the All.
        let contour = layers.iter().find(|l| l.id() == "contour-index").unwrap();
        match contour {
            Layer::Line { filter, .. } => assert_eq!(
                filter,
                &Filter::All(vec![
                    Filter::ZoomRange { min: 11, max: 22 },
                    Filter::Eq("is_index".into(), FilterValue::Bool(true)),
                ])
            ),
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn line_width_stops_lower_to_a_zoom_curve_in_px_times_extent_units() {
        let layers = parse_style_layers(&n50_topo(), "n50").unwrap();
        let vei = layers.iter().find(|l| l.id() == "road-vei").unwrap();
        match vei {
            Layer::Line { width, .. } => assert_eq!(
                width,
                &Paint::Zoom {
                    stops: vec![
                        ZoomStop {
                            zoom: 9.0,
                            value: 0.8 * EXTENT_UNITS_PER_PX
                        },
                        ZoomStop {
                            zoom: 12.0,
                            value: 1.6 * EXTENT_UNITS_PER_PX
                        },
                        ZoomStop {
                            zoom: 15.0,
                            value: 3.0 * EXTENT_UNITS_PER_PX
                        },
                    ],
                }
            ),
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn background_surfaces_separately_and_water_fills_strip() {
        assert_eq!(
            parse_style_background(&n50_topo()).unwrap(),
            Some(Color::rgb(0xf3, 0xf1, 0xea))
        );
        let layers = parse_style_layers(&n50_topo(), "n50").unwrap();
        assert!(layers.iter().any(|l| l.id() == "water"));
        let stripped = without_water_fill_layers(layers);
        assert!(!stripped.iter().any(|l| l.id() == "water"));
        // Lines/labels survive the strip; only water FILLS go.
        assert!(stripped.iter().any(|l| l.id() == "coastline"));
        assert_eq!(stripped.len(), 13);
    }

    #[test]
    fn match_interpolate_and_extrusion_constructs_lower() {
        let s = r##"{"layers":[
            {"id":"landuse","type":"fill","source-layer":"landuse","paint":{
                "fill-color":["match",["get","class"],
                    "wood","#a0c090",["grass","meadow"],"#b8d0a0","#cccccc"]}},
            {"id":"waterway","type":"line","source-layer":"waterway","paint":{
                "line-color":["interpolate",["linear"],["zoom"],8,"#a0c0e0",14,"#6090c0"],
                "line-width":["interpolate",["linear"],["zoom"],8,1,14,3]}},
            {"id":"b3d","type":"fill-extrusion","source-layer":"building","minzoom":13,
             "paint":{"fill-extrusion-color":"#d2bfae",
                      "fill-extrusion-height":["get","render_height"],
                      "fill-extrusion-base":["get","render_min_height"]}}
        ]}"##;
        let layers = parse_style_layers(s, "src").unwrap();

        match &layers[0] {
            Layer::Fill { color, .. } => assert_eq!(
                color,
                &Paint::Match {
                    property: "class".into(),
                    cases: vec![
                        MatchCase {
                            value: FilterValue::String("wood".into()),
                            result: Color::rgb(0xa0, 0xc0, 0x90),
                        },
                        MatchCase {
                            value: FilterValue::String("grass".into()),
                            result: Color::rgb(0xb8, 0xd0, 0xa0),
                        },
                        MatchCase {
                            value: FilterValue::String("meadow".into()),
                            result: Color::rgb(0xb8, 0xd0, 0xa0),
                        },
                    ],
                    default: Box::new(Color::rgb(0xcc, 0xcc, 0xcc)),
                }
            ),
            other => panic!("expected fill, got {other:?}"),
        }
        match &layers[1] {
            Layer::Line { color, width, .. } => {
                assert!(matches!(color, Paint::Zoom { stops } if stops.len() == 2));
                assert_eq!(
                    width,
                    &Paint::Zoom {
                        stops: vec![
                            ZoomStop {
                                zoom: 8.0,
                                value: EXTENT_UNITS_PER_PX
                            },
                            ZoomStop {
                                zoom: 14.0,
                                value: 3.0 * EXTENT_UNITS_PER_PX
                            },
                        ],
                    }
                );
            }
            other => panic!("expected line, got {other:?}"),
        }
        match &layers[2] {
            Layer::FillExtrusion {
                filter,
                height_m,
                height_property,
                min_height_property,
                ..
            } => {
                assert_eq!(filter, &Filter::ZoomRange { min: 13, max: 22 });
                assert_eq!(height_m, &Paint::Const(0.0));
                assert_eq!(height_property.as_deref(), Some("render_height"));
                assert_eq!(min_height_property.as_deref(), Some("render_min_height"));
            }
            other => panic!("expected fill-extrusion, got {other:?}"),
        }
    }

    #[test]
    fn compound_and_negated_filters_lower() {
        let s = r##"{"layers":[{"id":"x","type":"line","source-layer":"roads",
            "filter":["all",["==","class","street"],["!=","brunnel","tunnel"],
                      ["!in","kind","a","b"]],
            "paint":{"line-color":"#fff","line-width":1}}]}"##;
        let layers = parse_style_layers(s, "src").unwrap();
        match &layers[0] {
            Layer::Line { filter, .. } => assert_eq!(
                filter,
                &Filter::All(vec![
                    Filter::Eq("class".into(), FilterValue::String("street".into())),
                    Filter::Not(Box::new(Filter::Eq(
                        "brunnel".into(),
                        FilterValue::String("tunnel".into()),
                    ))),
                    Filter::Not(Box::new(Filter::In(
                        "kind".into(),
                        vec![
                            FilterValue::String("a".into()),
                            FilterValue::String("b".into()),
                        ],
                    ))),
                ])
            ),
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_constructs_still_fail_loud() {
        let bad_type = r##"{"layers":[{"id":"x","type":"circle","source-layer":"a"}]}"##;
        assert!(matches!(
            parse_style_layers(bad_type, "s"),
            Err(StyleError::Unsupported { .. })
        ));
        let bad_filter = r##"{"layers":[{"id":"x","type":"fill","source-layer":"water",
            "filter":["has","name"],"paint":{"fill-color":"#fff"}}]}"##;
        assert!(parse_style_layers(bad_filter, "s").is_err());
        let bad_interp = r##"{"layers":[{"id":"x","type":"line","source-layer":"w","paint":{
            "line-color":"#fff",
            "line-width":["interpolate",["exponential",2],["zoom"],8,1,14,3]}}]}"##;
        assert!(parse_style_layers(bad_interp, "s").is_err());
    }
}
