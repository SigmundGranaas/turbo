//! P6.2 fidelity gate for the MapLibre → Scene IR lowering.
//!
//! Ground truth is the engine's IR → core `VectorStyle` compilation
//! (`compile_vector_layer_style`, the exact lowering `reconcile` runs):
//! for the real house style (n50-topo) the compiled IR rules must select
//! the same paint for every probed feature at every zoom as the legacy
//! `parse_style` output — the style the desktop app rendered until now.
//!
//! ONE documented divergence is asserted rather than hidden: legacy
//! `line-width` `{stops}` functions. `parse_style` lowered them to
//! piecewise-CONSTANT zoom bands; the IR carries the curve and the engine
//! interpolates LINEARLY between stops (MapLibre's real semantics). The two
//! agree exactly at the stop zooms; between stops the IR width must lie
//! strictly between the neighbouring band widths.

use turbomap_core::vector::{Feature, GeomType, Geometry, Value};
use turbomap_core::{Color as CoreColor, Paint as CorePaint, Rule as CoreRule, VectorStyle};
use turbomap_engine::engine::compile_vector_layer_style;
use turbomap_scene::Layer;
use turbomap_style_maplibre::{
    parse_style, parse_style_background, parse_style_layers, without_water_fill_layers,
};

fn n50_topo() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tileserver/styles/n50-topo.json");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn feature(gt: GeomType, props: &[(&str, Value)]) -> Feature {
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
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    }
}

fn poly(props: &[(&str, &str)]) -> Feature {
    let p: Vec<(&str, Value)> = props
        .iter()
        .map(|(k, v)| (*k, Value::String((*v).to_string())))
        .collect();
    feature(GeomType::Polygon, &p)
}

fn linestr(props: &[(&str, Value)]) -> Feature {
    feature(GeomType::LineString, props)
}

fn point(props: &[(&str, &str)]) -> Feature {
    let p: Vec<(&str, Value)> = props
        .iter()
        .map(|(k, v)| (*k, Value::String((*v).to_string())))
        .collect();
    feature(GeomType::Point, &p)
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

/// Every feature shape the n50-topo style distinguishes, plus a
/// non-matching value per filtered source layer (the None/None case).
fn n50_probes() -> Vec<(&'static str, Feature)> {
    vec![
        ("landcover", poly(&[("class", "forest")])),
        ("landcover", poly(&[("class", "wetland")])),
        ("landcover", poly(&[("class", "scree")])),
        ("glacier", poly(&[])),
        ("water", poly(&[])),
        ("coastline", linestr(&[])),
        ("contour", linestr(&[("is_index", Value::Bool(true))])),
        ("contour", linestr(&[("is_index", Value::Bool(false))])),
        ("building", poly(&[])),
        ("transportation", linestr(&[("class", s("vei"))])),
        ("transportation", linestr(&[("class", s("skogsvei"))])),
        ("transportation", linestr(&[("class", s("traktorvei"))])),
        ("transportation", linestr(&[("class", s("sykkelvei"))])),
        ("transportation", linestr(&[("class", s("sti"))])),
        ("transportation", linestr(&[("class", s("skiloype"))])),
        ("transportation", linestr(&[("class", s("jernbane"))])),
        (
            "place",
            point(&[("kind", "summit"), ("name", "Galdhøpiggen")]),
        ),
        ("place", point(&[("kind", "named_place"), ("name", "Vika")])),
        ("place", point(&[("kind", "cabin"), ("name", "Hytta")])),
        (
            "place",
            point(&[("kind", "waterfeature"), ("name", "Osen")]),
        ),
        ("place", point(&[("kind", "other"), ("name", "X")])),
    ]
}

/// Compile IR layers exactly as the engine's reconcile does (declared MVT
/// source-layer names, device pixel ratio 1), concatenated in stack order
/// so core's first-match-wins sees the same ordering the app renders.
fn compile_at(layers: &[Layer], zoom: f64) -> VectorStyle {
    let mut rules: Vec<CoreRule> = Vec::new();
    for l in layers {
        let name = match l {
            Layer::Fill { source_layer, .. }
            | Layer::FillExtrusion { source_layer, .. }
            | Layer::Line { source_layer, .. }
            | Layer::Symbol { source_layer, .. } => source_layer.clone().unwrap_or_default(),
            other => panic!("unexpected non-vector layer {other:?}"),
        };
        let style = compile_vector_layer_style(l, name, zoom, 1.0).expect("vector layer compiles");
        rules.extend(style.rules);
    }
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules,
    }
}

/// When paints diverge, the ONLY sanctioned divergence is a line width
/// inside a legacy stop band: same colour, and the IR's linearly
/// interpolated width strictly between this band's width and the next's.
fn assert_banded_width_divergence(
    original: &VectorStyle,
    source_layer: &str,
    f: &Feature,
    z: u8,
    expected: &CorePaint,
    got: &CorePaint,
) {
    let (
        CorePaint::Line {
            color: ec,
            width: ew,
        },
        CorePaint::Line {
            color: gc,
            width: gw,
        },
    ) = (expected, got)
    else {
        panic!("paints diverge beyond line width at {source_layer} z{z}: {expected:?} vs {got:?}");
    };
    assert_eq!(ec, gc, "line colour must match at {source_layer} z{z}");
    // The next zoom where the original's banded width changes bounds the
    // permitted interpolation window.
    let next_w = (z..=22u8).find_map(|zz| {
        original
            .matching_rule(source_layer, f, zz)
            .and_then(|i| match original.rules[i].paint {
                CorePaint::Line { width, .. } if width != *ew => Some(width),
                _ => None,
            })
    });
    let Some(nw) = next_w else {
        panic!("width diverged with no following band at {source_layer} z{z}: {ew} vs {gw}");
    };
    let (lo, hi) = if *ew < nw { (*ew, nw) } else { (nw, *ew) };
    assert!(
        *gw > lo && *gw < hi,
        "IR width {gw} at {source_layer} z{z} must lie strictly between the \
         banded widths {lo}..{hi} (linear interpolation between stops)"
    );
}

/// Probe both styles feature-by-feature, zoom-by-zoom; return which rule
/// indices were exercised so callers can assert full coverage.
fn check_equivalence(original: &VectorStyle, layers: &[Layer]) -> (Vec<bool>, Vec<bool>) {
    let probes = n50_probes();
    let mut orig_hit = vec![false; original.rules.len()];
    let mut got_hit: Vec<bool> = Vec::new();
    for z in 0..=22u8 {
        let compiled = compile_at(layers, f64::from(z));
        if got_hit.is_empty() {
            got_hit = vec![false; compiled.rules.len()];
        }
        assert_eq!(
            got_hit.len(),
            compiled.rules.len(),
            "compiled rule count must be zoom-stable"
        );
        for (sl, f) in &probes {
            let e = original.matching_rule(sl, f, z);
            let g = compiled.matching_rule(sl, f, z);
            match (e, g) {
                (None, None) => {}
                (Some(ei), Some(gi)) => {
                    orig_hit[ei] = true;
                    got_hit[gi] = true;
                    let ep = &original.rules[ei].paint;
                    let gp = &compiled.rules[gi].paint;
                    if ep != gp {
                        assert_banded_width_divergence(original, sl, f, z, ep, gp);
                    }
                    // Interactivity policy matches too: parse_style never
                    // set it; the engine only sets it for icon symbols.
                    assert_eq!(
                        original.rules[ei].interactive, compiled.rules[gi].interactive,
                        "interactive flag at {sl} z{z}"
                    );
                }
                (e, g) => panic!(
                    "probe {sl} {:?} at z{z}: original matched {e:?}, IR compiled {g:?}",
                    f.properties
                ),
            }
        }
    }
    (orig_hit, got_hit)
}

#[test]
fn n50_topo_ir_layers_compile_to_the_parse_style_rules() {
    let json = n50_topo();
    let original = parse_style(&json).expect("parse_style");
    let layers = parse_style_layers(&json, "n50").expect("parse_style_layers");

    let (orig_hit, got_hit) = check_equivalence(&original, &layers);

    // The probe set must exercise every rule on BOTH sides — otherwise the
    // gate silently stops covering a style layer when the style grows.
    for (i, hit) in orig_hit.iter().enumerate() {
        assert!(
            hit,
            "probe set never selected original rule #{i} ({:?} on `{}`) — extend n50_probes",
            original.rules[i].filter, original.rules[i].source_layer
        );
    }
    let compiled = compile_at(&layers, 12.0);
    for (i, hit) in got_hit.iter().enumerate() {
        assert!(
            hit,
            "probe set never selected compiled rule #{i} ({:?} on `{}`) — extend n50_probes",
            compiled.rules[i].filter, compiled.rules[i].source_layer
        );
    }

    // Background is surfaced separately (the Scene has no background layer).
    let bg = parse_style_background(&json).unwrap().expect("background");
    assert_eq!(
        (bg.r, bg.g, bg.b, bg.a),
        (
            original.background.r,
            original.background.g,
            original.background.b,
            original.background.a
        )
    );
}

#[test]
fn water_fill_strip_matches_without_water_fills() {
    let json = n50_topo();
    let original = parse_style(&json).unwrap().without_water_fills();
    let layers = without_water_fill_layers(parse_style_layers(&json, "n50").unwrap());

    // Same probe-by-probe behaviour on the stripped styles…
    check_equivalence(&original, &layers);

    // …and the water polygon specifically no longer matches on either side.
    let water = poly(&[]);
    assert_eq!(original.matching_rule("water", &water, 10), None);
    assert_eq!(
        compile_at(&layers, 10.0).matching_rule("water", &water, 10),
        None
    );
}

/// Constructs `parse_style` cannot express (match, linear interpolate,
/// fill-extrusion) are gated against hand-computed engine output instead.
#[test]
fn match_interpolate_and_extrusion_compile_to_expected_rules() {
    let fixture = r##"{"layers":[
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
    let layers = parse_style_layers(fixture, "src").unwrap();
    let px = turbomap_style_maplibre::EXTENT_UNITS_PER_PX;

    // A data-driven match expands to one rule per case + the default, in
    // case order, so core's first-match-wins picks the specific class.
    let landuse_probe = |class: &str| poly(&[("class", class)]);
    let compiled = compile_at(&layers, 8.0);
    let color_of = |f: &Feature| match compiled.rules[compiled
        .matching_rule("landuse", f, 8)
        .expect("landuse match")]
    .paint
    {
        CorePaint::Fill { color } => color,
        ref other => panic!("expected fill, got {other:?}"),
    };
    assert_eq!(
        color_of(&landuse_probe("wood")),
        CoreColor::rgb(0xa0, 0xc0, 0x90)
    );
    assert_eq!(
        color_of(&landuse_probe("grass")),
        CoreColor::rgb(0xb8, 0xd0, 0xa0)
    );
    assert_eq!(
        color_of(&landuse_probe("meadow")),
        CoreColor::rgb(0xb8, 0xd0, 0xa0)
    );
    assert_eq!(
        color_of(&landuse_probe("farm")),
        CoreColor::rgb(0xcc, 0xcc, 0xcc)
    );

    // Linear zoom interpolation: exact at stops, midpoint halfway (colour
    // channels round-lerp; width in px × extent units).
    let ww = linestr(&[]);
    let line_at = |zoom: f64, z: u8| {
        let c = compile_at(&layers, zoom);
        match c.rules[c.matching_rule("waterway", &ww, z).expect("waterway")].paint {
            CorePaint::Line { color, width } => (color, width),
            ref other => panic!("expected line, got {other:?}"),
        }
    };
    assert_eq!(line_at(8.0, 8), (CoreColor::rgb(0xa0, 0xc0, 0xe0), px));
    assert_eq!(
        line_at(14.0, 14),
        (CoreColor::rgb(0x60, 0x90, 0xc0), 3.0 * px)
    );
    assert_eq!(
        line_at(11.0, 11),
        (CoreColor::rgb(0x80, 0xa8, 0xd0), 2.0 * px)
    );

    // Fill-extrusion: per-feature height/base properties, zoom-windowed.
    let b = poly(&[]);
    let c13 = compile_at(&layers, 13.0);
    assert_eq!(c13.matching_rule("building", &b, 12), None, "below minzoom");
    let idx = c13.matching_rule("building", &b, 13).expect("extrusion");
    let rule = &c13.rules[idx];
    assert_eq!((rule.min_zoom, rule.max_zoom), (13, 22));
    match &rule.paint {
        CorePaint::FillExtrusion {
            color,
            height_m,
            height_property,
            min_height_property,
        } => {
            assert_eq!(*color, CoreColor::rgb(0xd2, 0xbf, 0xae));
            assert_eq!(*height_m, 0.0);
            assert_eq!(height_property.as_deref(), Some("render_height"));
            assert_eq!(min_height_property.as_deref(), Some("render_min_height"));
        }
        other => panic!("expected fill-extrusion, got {other:?}"),
    }
}
