//! Paint evaluation, interpolation, and serde round-trips.

use turbomap_scene::style::{FilterValue, MatchCase, ZoomStop};
use turbomap_scene::{Color, Filter, Interpolate, Paint};

#[test]
fn const_paint_ignores_zoom() {
    let p = Paint::Const(3.5f32);
    assert_eq!(p.at(0.0), 3.5);
    assert_eq!(p.at(22.0), 3.5);
}

#[test]
fn zoom_curve_interpolates_and_clamps() {
    let p = Paint::Zoom {
        stops: vec![
            ZoomStop { zoom: 10.0, value: 1.0f32 },
            ZoomStop { zoom: 14.0, value: 5.0f32 },
        ],
    };
    assert_eq!(p.at(8.0), 1.0, "below first stop clamps");
    assert_eq!(p.at(10.0), 1.0, "at first stop");
    assert_eq!(p.at(12.0), 3.0, "midpoint interpolates");
    assert_eq!(p.at(14.0), 5.0, "at last stop");
    assert_eq!(p.at(20.0), 5.0, "above last stop clamps");
}

#[test]
fn zoom_curve_with_three_stops_picks_right_segment() {
    let p = Paint::Zoom {
        stops: vec![
            ZoomStop { zoom: 0.0, value: 0.0f64 },
            ZoomStop { zoom: 10.0, value: 10.0 },
            ZoomStop { zoom: 20.0, value: 0.0 },
        ],
    };
    assert_eq!(p.at(5.0), 5.0);
    assert_eq!(p.at(15.0), 5.0);
}

#[test]
fn color_interpolates_per_channel() {
    let c = Color::interpolate(Color::rgb(0, 0, 0), Color::rgb(255, 100, 50), 0.5);
    assert_eq!(c, Color::rgba(128, 50, 25, 255));
}

#[test]
fn paint_color_zoom_curve() {
    let p = Paint::Zoom {
        stops: vec![
            ZoomStop { zoom: 0.0, value: Color::rgb(0, 0, 0) },
            ZoomStop { zoom: 10.0, value: Color::rgb(100, 0, 0) },
        ],
    };
    assert_eq!(p.at(5.0), Color::rgb(50, 0, 0));
}

#[test]
fn paint_roundtrips_through_json() {
    let p = Paint::Zoom {
        stops: vec![
            ZoomStop { zoom: 10.0, value: 2.0f32 },
            ZoomStop { zoom: 16.0, value: 6.0f32 },
        ],
    };
    let json = serde_json::to_string(&p).unwrap();
    let back: Paint<f32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}

#[test]
fn match_paint_returns_default_without_feature_context() {
    let p: Paint<Color> = Paint::Match {
        property: "kind".to_string(),
        cases: vec![MatchCase {
            value: FilterValue::String("path".to_string()),
            result: Color::rgb(0, 200, 0),
        }],
        default: Box::new(Color::rgb(80, 80, 80)),
    };
    // No feature context at a bare zoom → the default.
    assert_eq!(p.at(12.0), Color::rgb(80, 80, 80));
    assert!(p.is_data_driven());
    assert!(!Paint::Const(Color::rgb(1, 2, 3)).is_data_driven());
}

#[test]
fn match_paint_roundtrips_through_json() {
    let p: Paint<f32> = Paint::Match {
        property: "level".to_string(),
        cases: vec![
            MatchCase { value: FilterValue::Number(1.0), result: 2.0 },
            MatchCase { value: FilterValue::Bool(true), result: 5.0 },
        ],
        default: Box::new(1.0),
    };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"match\""), "{json}");
    let back: Paint<f32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}

#[test]
fn filter_value_is_untagged_in_json() {
    // FilterValue is untagged so styles read naturally: "highway", 3, true.
    let f = Filter::In(
        "kind".to_string(),
        vec![
            FilterValue::String("path".to_string()),
            FilterValue::Number(3.0),
            FilterValue::Bool(true),
        ],
    );
    let json = serde_json::to_string(&f).unwrap();
    assert!(json.contains("\"path\""), "{json}");
    assert!(json.contains("3.0") || json.contains("3"), "{json}");
    let back: Filter = serde_json::from_str(&json).unwrap();
    assert_eq!(back, f);
}

#[test]
fn nested_filter_roundtrips() {
    let f = Filter::All(vec![
        Filter::Eq("a".into(), FilterValue::Bool(true)),
        Filter::Not(Box::new(Filter::Eq("b".into(), FilterValue::Number(1.0)))),
        Filter::Any(vec![Filter::Always]),
    ]);
    let json = serde_json::to_string(&f).unwrap();
    let back: Filter = serde_json::from_str(&json).unwrap();
    assert_eq!(back, f);
}
