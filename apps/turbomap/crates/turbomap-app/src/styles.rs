//! The desktop demo's hand-authored styles as **Scene IR layer lists**
//! (plan P6.2) — the IR replacements for `app.rs`'s hand-built core
//! `VectorStyle`s (`water_only_style` / `versatiles_demo_style`). The
//! engine compiles these into the core style itself
//! (`turbomap_engine::engine::compile_vector_layer_style`), so the app
//! only ever authors WHAT the map shows, never renderer rules.
//!
//! Fidelity is gated by the tests below: the compiled IR must reproduce
//! the original `VectorStyle` rules (transcribed here as the ground
//! truth). Two DOCUMENTED deviations, asserted rather than hidden:
//!
//! 1. **`interactive`**: the IR has no per-layer interactivity knob — it
//!    is engine policy (only icon-carrying symbol rules retain features).
//!    The original style marked buildings/streets/labels interactive; the
//!    compiled rules are not. Desktop hit-testing on those layers returns
//!    with plan P6.4 (hit-testing through the production bindings).
//! 2. **streets catch-all**: the original relied on core's
//!    first-match-wins to keep the grey minor-road rule off
//!    motorway/primary features. Each IR layer compiles to its own style
//!    (every matching layer draws — the MapLibre model), so the minor
//!    tier excludes the named tiers explicitly. Same feature → same
//!    paint, proven by the probe test.

use turbomap_scene::{Color, Filter, FilterValue, Layer, Paint, SymbolPlacement, TextAnchor};

fn kind_in(values: &[&str]) -> Filter {
    Filter::In(
        "kind".to_string(),
        values
            .iter()
            .map(|v| FilterValue::String((*v).to_string()))
            .collect(),
    )
}

fn kind_eq(value: &str) -> Filter {
    Filter::Eq("kind".to_string(), FilterValue::String(value.to_string()))
}

/// Water-only debug style (TURBO_WATER_ONLY=1): just the water-body
/// fills, nothing else. Covers both schemas — OMT/kart-api "water" and
/// VersaTiles "ocean" + "water_polygons" (the same set
/// `without_water_fills` strips).
pub(crate) fn water_only_layers(source: &str) -> Vec<Layer> {
    let water = |layer: &str| Layer::Fill {
        id: layer.to_string(),
        source: source.to_string(),
        source_layer: Some(layer.to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(0x9E, 0xC2, 0xDF)),
        opacity: Paint::Const(1.0),
    };
    vec![water("water"), water("ocean"), water("water_polygons")]
}

/// A minimal but readable overlay style for VersaTiles' Shortbread schema
/// (pluralised layer names, `kind` instead of `class`): rivers, buildings,
/// a three-tier road hierarchy, boundaries, and zoom-tiered place/street
/// labels. Broad-area fills are intentionally omitted — the raster basemap
/// underneath shows through wherever no vector rule paints.
pub(crate) fn versatiles_demo_layers(source: &str) -> Vec<Layer> {
    let src = source.to_string();
    let line = |id: &str, source_layer: &str, filter: Filter, color: Color, width: f32| -> Layer {
        Layer::Line {
            id: id.to_string(),
            source: src.clone(),
            source_layer: Some(source_layer.to_string()),
            filter,
            color: Paint::Const(color),
            width: Paint::Const(width),
            dash_array: None,
        }
    };
    let label = |id: &str,
                 source_layer: &str,
                 filter: Filter,
                 size: f32,
                 color: Color,
                 weight: f32|
     -> Layer {
        Layer::Symbol {
            id: id.to_string(),
            source: src.clone(),
            source_layer: Some(source_layer.to_string()),
            filter,
            text_field: "name".to_string(),
            text_size: Paint::Const(size),
            color: Paint::Const(color),
            halo_color: Paint::Const(Color::rgb(0xff, 0xff, 0xff)),
            halo_width: Paint::Const(1.0),
            sort_key: None,
            placement: SymbolPlacement::Point,
            icon_image: None,
            icon_size: Paint::Const(24.0),
            icon_color: Paint::Const(Color::rgb(70, 78, 92)),
            text_anchor: TextAnchor::Center,
            letter_spacing: 0.0,
            font_weight: weight,
        }
    };

    vec![
        // Rivers / streams.
        line(
            "water-lines",
            "water_lines",
            Filter::Always.within_zoom(8, 22),
            Color::rgb(0x9E, 0xC2, 0xDF),
            20.0,
        ),
        // Buildings (high zoom only).
        Layer::Fill {
            id: "buildings".to_string(),
            source: src.clone(),
            source_layer: Some("buildings".to_string()),
            filter: Filter::Always.within_zoom(14, 22),
            color: Paint::Const(Color::rgb(0xDC, 0xD2, 0xC1)),
            opacity: Paint::Const(1.0),
        },
        // Streets: motorways/trunks emphasised, primary tier middle,
        // everything-else at high zoom. Tiers are explicit layers, so the
        // minor tier must EXCLUDE the named tiers (each IR layer draws
        // independently — no cross-layer first-match-wins).
        line(
            "streets-major",
            "streets",
            kind_in(&["motorway", "trunk"]).within_zoom(6, 22),
            Color::rgb(0xE8, 0x9C, 0x4C),
            35.0,
        ),
        line(
            "streets-mid",
            "streets",
            kind_in(&["primary", "secondary", "tertiary"]).within_zoom(8, 22),
            Color::rgb(0xCE, 0xB9, 0x8B),
            22.0,
        ),
        line(
            "streets-minor",
            "streets",
            Filter::Not(Box::new(kind_in(&[
                "motorway",
                "trunk",
                "primary",
                "secondary",
                "tertiary",
            ])))
            .within_zoom(11, 22),
            Color::rgb(0xBD, 0xB3, 0xA1),
            12.0,
        ),
        // Country / state boundaries.
        line(
            "boundaries",
            "boundaries",
            Filter::Always,
            Color::rgb(0x70, 0x60, 0x60),
            10.0,
        ),
        // Place labels, zoom-tiered: country/state at low zoom,
        // city/town/village at higher zoom.
        label(
            "place-country",
            "place_labels",
            kind_eq("country").within_zoom(2, 6),
            18.0,
            Color::rgb(0x33, 0x33, 0x33),
            1.3,
        ),
        label(
            "place-state",
            "place_labels",
            kind_in(&["state", "province"]).within_zoom(4, 8),
            14.0,
            Color::rgb(0x44, 0x44, 0x44),
            1.3,
        ),
        label(
            "place-city",
            "place_labels",
            kind_eq("city").within_zoom(6, 14),
            15.0,
            Color::rgb(0x22, 0x22, 0x22),
            1.3,
        ),
        label(
            "place-town",
            "place_labels",
            kind_eq("town").within_zoom(9, 14),
            12.0,
            Color::rgb(0x33, 0x33, 0x33),
            1.3,
        ),
        label(
            "place-village",
            "place_labels",
            kind_in(&["village", "suburb", "neighbourhood"]).within_zoom(12, 14),
            11.0,
            Color::rgb(0x44, 0x44, 0x44),
            1.3,
        ),
        // Street labels (separate Shortbread layer).
        label(
            "street-labels",
            "street_labels",
            Filter::Always.within_zoom(14, 22),
            10.0,
            Color::rgb(0x55, 0x55, 0x55),
            0.7,
        ),
    ]
}

// ---------------------------------------------------------------------------
// P6.2 fidelity gate: the engine-compiled IR must reproduce the original
// hand-built core VectorStyles (transcribed below as the ground truth —
// app.rs's water_only_style / versatiles_demo_style, which the scene-host
// rewrite deletes).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use turbomap_core::style::{
        Color as CoreColor, Filter as CoreFilter, Paint as CorePaint, Rule, VectorStyle,
    };
    use turbomap_core::vector::{Feature, GeomType, Geometry, Value};
    use turbomap_engine::engine::compile_vector_layer_style;

    /// Compile the IR exactly as the engine's reconcile does (declared MVT
    /// source-layer names, pixel ratio 1), concatenated in stack order.
    fn compile(layers: &[Layer]) -> VectorStyle {
        let mut rules: Vec<Rule> = Vec::new();
        for l in layers {
            let name = match l {
                Layer::Fill { source_layer, .. }
                | Layer::Line { source_layer, .. }
                | Layer::Symbol { source_layer, .. } => source_layer.clone().unwrap_or_default(),
                other => panic!("unexpected layer {other:?}"),
            };
            // The hand styles carry no zoom-curve paints, so the compile
            // zoom is irrelevant; 10.0 is arbitrary.
            let style = compile_vector_layer_style(l, name, 10.0, 1.0).expect("vector layer");
            rules.extend(style.rules);
        }
        VectorStyle {
            background: CoreColor::rgba(0, 0, 0, 0),
            rules,
        }
    }

    fn feature(gt: GeomType, props: &[(&str, &str)]) -> Feature {
        Feature {
            id: 0,
            geom_type: gt,
            geometry: match gt {
                GeomType::Point => Geometry::Point(vec![]),
                GeomType::LineString => Geometry::LineString(vec![]),
                _ => Geometry::Polygon(vec![]),
            },
            properties: props
                .iter()
                .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
                .collect(),
        }
    }

    fn text_paint(size: f32, color: CoreColor, weight: f32) -> CorePaint {
        CorePaint::Text {
            text_field: "name".to_string(),
            font_size_px: size,
            color,
            halo_color: CoreColor::rgb(0xff, 0xff, 0xff),
            halo_width: 1.0,
            rank_field: None,
            along_line: false,
            icon: None,
            left_anchor: false,
            letter_spacing: 0.0,
            weight,
        }
    }

    /// The ORIGINAL `versatiles_demo_style()` rules from app.rs, verbatim.
    fn original_versatiles() -> VectorStyle {
        let rule = |source_layer: &str,
                    filter: CoreFilter,
                    paint: CorePaint,
                    min_zoom: u8,
                    max_zoom: u8,
                    interactive: bool| Rule {
            source_layer: source_layer.to_string(),
            filter,
            paint,
            min_zoom,
            max_zoom,
            interactive,
        };
        let line = |c: CoreColor, width: f32| CorePaint::Line { color: c, width };
        let one = |v: &str| CoreFilter::Eq("kind".into(), v.into());
        let any = |vs: &[&str]| {
            CoreFilter::In("kind".into(), vs.iter().map(|v| (*v).to_string()).collect())
        };
        VectorStyle {
            background: CoreColor::rgba(0, 0, 0, 0),
            rules: vec![
                rule(
                    "water_lines",
                    CoreFilter::Always,
                    line(CoreColor::rgb(0x9E, 0xC2, 0xDF), 20.0),
                    8,
                    22,
                    false,
                ),
                rule(
                    "buildings",
                    CoreFilter::Always,
                    CorePaint::Fill {
                        color: CoreColor::rgb(0xDC, 0xD2, 0xC1),
                    },
                    14,
                    22,
                    true,
                ),
                rule(
                    "streets",
                    any(&["motorway", "trunk"]),
                    line(CoreColor::rgb(0xE8, 0x9C, 0x4C), 35.0),
                    6,
                    22,
                    true,
                ),
                rule(
                    "streets",
                    any(&["primary", "secondary", "tertiary"]),
                    line(CoreColor::rgb(0xCE, 0xB9, 0x8B), 22.0),
                    8,
                    22,
                    true,
                ),
                rule(
                    "streets",
                    CoreFilter::Always,
                    line(CoreColor::rgb(0xBD, 0xB3, 0xA1), 12.0),
                    11,
                    22,
                    true,
                ),
                rule(
                    "boundaries",
                    CoreFilter::Always,
                    line(CoreColor::rgb(0x70, 0x60, 0x60), 10.0),
                    0,
                    22,
                    false,
                ),
                rule(
                    "place_labels",
                    one("country"),
                    text_paint(18.0, CoreColor::rgb(0x33, 0x33, 0x33), 1.3),
                    2,
                    6,
                    true,
                ),
                rule(
                    "place_labels",
                    any(&["state", "province"]),
                    text_paint(14.0, CoreColor::rgb(0x44, 0x44, 0x44), 1.3),
                    4,
                    8,
                    true,
                ),
                rule(
                    "place_labels",
                    one("city"),
                    text_paint(15.0, CoreColor::rgb(0x22, 0x22, 0x22), 1.3),
                    6,
                    14,
                    true,
                ),
                rule(
                    "place_labels",
                    one("town"),
                    text_paint(12.0, CoreColor::rgb(0x33, 0x33, 0x33), 1.3),
                    9,
                    14,
                    true,
                ),
                rule(
                    "place_labels",
                    any(&["village", "suburb", "neighbourhood"]),
                    text_paint(11.0, CoreColor::rgb(0x44, 0x44, 0x44), 1.3),
                    12,
                    14,
                    true,
                ),
                rule(
                    "street_labels",
                    CoreFilter::Always,
                    text_paint(10.0, CoreColor::rgb(0x55, 0x55, 0x55), 0.7),
                    14,
                    22,
                    true,
                ),
            ],
        }
    }

    /// The ORIGINAL `water_only_style()` rules from app.rs, verbatim.
    fn original_water_only() -> VectorStyle {
        let water = |layer: &str| Rule {
            source_layer: layer.into(),
            filter: CoreFilter::Always,
            paint: CorePaint::Fill {
                color: CoreColor::rgb(0x9E, 0xC2, 0xDF),
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        };
        VectorStyle {
            background: CoreColor::rgba(0, 0, 0, 0),
            rules: vec![water("water"), water("ocean"), water("water_polygons")],
        }
    }

    #[test]
    fn water_only_layers_compile_to_the_original_style_exactly() {
        let expected = original_water_only();
        let got = compile(&water_only_layers("osm"));
        assert_eq!(got.rules.len(), expected.rules.len());
        for (i, (e, g)) in expected.rules.iter().zip(&got.rules).enumerate() {
            // core Rule has no PartialEq; the Debug form covers every field.
            assert_eq!(format!("{e:?}"), format!("{g:?}"), "rule #{i}");
        }
        assert_eq!(got.background, expected.background);
    }

    #[test]
    fn versatiles_layers_compile_to_the_original_rules() {
        let expected = original_versatiles();
        let got = compile(&versatiles_demo_layers("osm"));
        assert_eq!(got.rules.len(), expected.rules.len());
        for (i, (e, g)) in expected.rules.iter().zip(&got.rules).enumerate() {
            assert_eq!(e.source_layer, g.source_layer, "rule #{i} source layer");
            assert_eq!(
                (e.min_zoom, e.max_zoom),
                (g.min_zoom, g.max_zoom),
                "rule #{i} zoom window"
            );
            assert_eq!(e.paint, g.paint, "rule #{i} paint");
            // DOCUMENTED DEVIATION (2): the streets catch-all becomes an
            // explicit exclusion (rule #4). Every other filter is verbatim.
            if i == 4 {
                assert_eq!(
                    g.filter,
                    CoreFilter::Not(Box::new(CoreFilter::In(
                        "kind".into(),
                        ["motorway", "trunk", "primary", "secondary", "tertiary"]
                            .iter()
                            .map(|v| (*v).to_string())
                            .collect(),
                    ))),
                    "rule #4 is the explicit minor-street exclusion"
                );
            } else {
                assert_eq!(e.filter, g.filter, "rule #{i} filter");
            }
        }
        // DOCUMENTED DEVIATION (1): interactivity is engine policy now —
        // no icon symbols in this style, so nothing compiled interactive.
        // (The original marked buildings/streets/labels interactive; those
        // hit-tests return via plan P6.4.)
        assert!(expected.rules.iter().any(|r| r.interactive));
        assert!(got.rules.iter().all(|r| !r.interactive));
    }

    /// The exclusion rewrite must select the SAME paint for every street
    /// kind at every zoom that the original first-match-wins stack did —
    /// including features with no `kind` at all.
    #[test]
    fn street_tiers_pick_the_same_paint_as_first_match_wins() {
        let expected = original_versatiles();
        let got = compile(&versatiles_demo_layers("osm"));
        let kinds: [&[(&str, &str)]; 8] = [
            &[("kind", "motorway")],
            &[("kind", "trunk")],
            &[("kind", "primary")],
            &[("kind", "secondary")],
            &[("kind", "tertiary")],
            &[("kind", "residential")],
            &[("kind", "service")],
            &[], // kind-less feature: catch-all grey in both worlds
        ];
        for props in kinds {
            let f = feature(GeomType::LineString, props);
            for z in 0..=22u8 {
                let e = expected
                    .matching_rule("streets", &f, z)
                    .map(|i| expected.rules[i].paint.clone());
                let g = got
                    .matching_rule("streets", &f, z)
                    .map(|i| got.rules[i].paint.clone());
                assert_eq!(e, g, "streets {props:?} at z{z}");
            }
        }
    }

    /// Zoom windows drive label tiers; probe every tier across all zooms.
    #[test]
    fn label_tiers_match_the_original_across_all_zooms() {
        let expected = original_versatiles();
        let got = compile(&versatiles_demo_layers("osm"));
        for kind in [
            "country",
            "state",
            "province",
            "city",
            "town",
            "village",
            "suburb",
            "neighbourhood",
            "hamlet", // matches nothing in either style
        ] {
            let f = feature(GeomType::Point, &[("kind", kind), ("name", "X")]);
            for z in 0..=22u8 {
                let e = expected
                    .matching_rule("place_labels", &f, z)
                    .map(|i| expected.rules[i].paint.clone());
                let g = got
                    .matching_rule("place_labels", &f, z)
                    .map(|i| got.rules[i].paint.clone());
                assert_eq!(e, g, "place_labels kind={kind} at z{z}");
            }
        }
    }
}
