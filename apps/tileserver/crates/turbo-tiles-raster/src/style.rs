//! Raster-side reading of the house MapLibre style (`styles/n50-topo.json`).
//!
//! Same subset as `turbomap-style-maplibre`, but where the GPU renderer
//! lowers `line-width` stops to zoom-banded rules, the raster path knows the
//! exact zoom it is rasterising at, so widths interpolate continuously.
//! Filters match against the per-feature `jsonb` attributes the SQL fetch
//! returns, so booleans/numbers compare with their native types.

use serde_json::Value as Json;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

#[derive(Debug, Clone)]
pub enum Width {
    Const(f32),
    /// Legacy `{"stops": [[zoom, px], …]}` — linearly interpolated.
    Stops(Vec<(f32, f32)>),
}

impl Width {
    /// Width in px at a (fractional) zoom, clamped to the outer stops.
    pub fn at(&self, zoom: f32) -> f32 {
        match self {
            Width::Const(w) => *w,
            Width::Stops(stops) => {
                let first = stops.first().expect("non-empty stops");
                let last = stops.last().expect("non-empty stops");
                if zoom <= first.0 {
                    return first.1;
                }
                if zoom >= last.0 {
                    return last.1;
                }
                for pair in stops.windows(2) {
                    let (z0, w0) = pair[0];
                    let (z1, w1) = pair[1];
                    if zoom >= z0 && zoom <= z1 {
                        let t = (zoom - z0) / (z1 - z0);
                        return w0 + t * (w1 - w0);
                    }
                }
                last.1
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PaintKind {
    Fill { color: Rgba },
    Line { color: Rgba, width: Width },
    Text { field: String, size_px: f32, color: Rgba },
}

#[derive(Debug, Clone)]
pub enum Filter {
    Always,
    Eq(String, Json),
    In(String, Vec<Json>),
}

impl Filter {
    /// Match against the feature's `jsonb` attribute object.
    pub fn matches(&self, attrs: &Json) -> bool {
        match self {
            Filter::Always => true,
            Filter::Eq(k, v) => attrs.get(k) == Some(v),
            Filter::In(k, vs) => attrs.get(k).map(|a| vs.contains(a)).unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StyleLayer {
    pub id: String,
    pub source_layer: String,
    pub filter: Filter,
    pub min_zoom: u8,
    /// Inclusive (the spec's exclusive `maxzoom` is converted at parse).
    pub max_zoom: u8,
    pub paint: PaintKind,
}

#[derive(Debug, Clone)]
pub struct RasterStyle {
    pub background: Rgba,
    pub layers: Vec<StyleLayer>,
}

#[derive(Debug, thiserror::Error)]
pub enum StyleError {
    #[error("style JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("style layer `{0}`: {1}")]
    Unsupported(String, String),
}

/// The same embedded document the `/v1/basemap/style.json` endpoint serves.
const EMBEDDED_STYLE: &str = include_str!("../../../styles/n50-topo.json");

impl RasterStyle {
    /// Disk copy (live-editable when running from the repo) wins over the
    /// embedded fallback — mirrors the style endpoint's behaviour.
    pub fn load_or_default() -> Result<Self, StyleError> {
        let text = std::fs::read_to_string("styles/n50-topo.json")
            .unwrap_or_else(|_| EMBEDDED_STYLE.to_string());
        Self::parse(&text)
    }

    pub fn parse(json: &str) -> Result<Self, StyleError> {
        let doc: Json = serde_json::from_str(json)?;
        let mut background = Rgba::rgb(255, 255, 255);
        let mut layers = Vec::new();

        for layer in doc["layers"].as_array().cloned().unwrap_or_default() {
            let id = layer["id"].as_str().unwrap_or("<unnamed>").to_string();
            let ty = layer["type"].as_str().unwrap_or_default();
            let paint = &layer["paint"];
            match ty {
                "background" => {
                    background = parse_color(&id, &paint["background-color"])?;
                    continue;
                }
                "fill" | "line" | "symbol" => {}
                // The raster path computes hillshade itself from the DEM and
                // has no raster sources, so it ignores `hillshade`/`raster`
                // layers in the shared style rather than rejecting them.
                "hillshade" | "raster" => continue,
                other => {
                    return Err(StyleError::Unsupported(id, format!("layer type `{other}`")))
                }
            }
            let source_layer = layer["source-layer"]
                .as_str()
                .ok_or_else(|| StyleError::Unsupported(id.clone(), "missing source-layer".into()))?
                .to_string();
            let kind = match ty {
                "fill" => PaintKind::Fill {
                    color: parse_color(&id, &paint["fill-color"])?,
                },
                "line" => PaintKind::Line {
                    color: parse_color(&id, &paint["line-color"])?,
                    width: parse_width(&id, &paint["line-width"])?,
                },
                "symbol" => {
                    let field = layer["layout"]["text-field"]
                        .as_str()
                        .and_then(|f| f.strip_prefix('{'))
                        .and_then(|f| f.strip_suffix('}'))
                        .ok_or_else(|| {
                            StyleError::Unsupported(id.clone(), "text-field (want `{prop}`)".into())
                        })?
                        .to_string();
                    PaintKind::Text {
                        field,
                        size_px: layer["layout"]["text-size"].as_f64().unwrap_or(16.0) as f32,
                        color: match paint.get("text-color") {
                            Some(c) => parse_color(&id, c)?,
                            None => Rgba::rgb(0, 0, 0),
                        },
                    }
                }
                _ => unreachable!(),
            };
            layers.push(StyleLayer {
                source_layer,
                filter: parse_filter(&id, layer.get("filter"))?,
                min_zoom: layer["minzoom"].as_u64().unwrap_or(0) as u8,
                max_zoom: layer["maxzoom"]
                    .as_u64()
                    .map(|z| (z as u8).saturating_sub(1))
                    .unwrap_or(22),
                paint: kind,
                id,
            });
        }
        Ok(RasterStyle { background, layers })
    }
}

fn parse_width(id: &str, v: &Json) -> Result<Width, StyleError> {
    match v {
        Json::Null => Ok(Width::Const(1.0)),
        n if n.is_number() => Ok(Width::Const(n.as_f64().unwrap() as f32)),
        Json::Object(o) => {
            let stops = o
                .get("stops")
                .and_then(|s| s.as_array())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| StyleError::Unsupported(id.into(), "width without stops".into()))?;
            let pairs = stops
                .iter()
                .map(|p| {
                    Some((p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| StyleError::Unsupported(id.into(), "malformed stops".into()))?;
            Ok(Width::Stops(pairs))
        }
        other => Err(StyleError::Unsupported(id.into(), format!("line-width `{other}`"))),
    }
}

fn parse_filter(id: &str, f: Option<&Json>) -> Result<Filter, StyleError> {
    let Some(f) = f else { return Ok(Filter::Always) };
    let arr = f
        .as_array()
        .ok_or_else(|| StyleError::Unsupported(id.into(), "filter not an array".into()))?;
    let op = arr.first().and_then(|o| o.as_str()).unwrap_or_default();
    let key = match arr.get(1) {
        Some(Json::String(s)) => s.clone(),
        Some(Json::Array(a)) if a.first().and_then(|g| g.as_str()) == Some("get") => a
            .get(1)
            .and_then(|k| k.as_str())
            .map(str::to_string)
            .ok_or_else(|| StyleError::Unsupported(id.into(), "get key".into()))?,
        _ => return Err(StyleError::Unsupported(id.into(), "filter key".into())),
    };
    match op {
        "==" => Ok(Filter::Eq(
            key,
            arr.get(2)
                .cloned()
                .ok_or_else(|| StyleError::Unsupported(id.into(), "== value".into()))?,
        )),
        "in" => Ok(Filter::In(key, arr[2..].to_vec())),
        other => Err(StyleError::Unsupported(id.into(), format!("filter op `{other}`"))),
    }
}

fn parse_color(id: &str, v: &Json) -> Result<Rgba, StyleError> {
    let s = v
        .as_str()
        .ok_or_else(|| StyleError::Unsupported(id.into(), format!("color `{v}`")))?;
    let hex = s
        .trim()
        .strip_prefix('#')
        .ok_or_else(|| StyleError::Unsupported(id.into(), format!("color `{s}`")))?;
    let h = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let parsed = match hex.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|v| v * 17);
            (|| Some(Rgba { r: d(0)?, g: d(1)?, b: d(2)?, a: 255 }))()
        }
        6 => (|| Some(Rgba { r: h(0)?, g: h(2)?, b: h(4)?, a: 255 }))(),
        8 => (|| Some(Rgba { r: h(0)?, g: h(2)?, b: h(4)?, a: h(6)? }))(),
        _ => None,
    };
    parsed.ok_or_else(|| StyleError::Unsupported(id.into(), format!("color `{s}`")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_embedded_house_style() {
        let style = RasterStyle::load_or_default().expect("parses");
        assert_eq!(style.background, Rgba::rgb(0xf3, 0xf1, 0xea));
        for sl in ["water", "contour", "transportation", "place", "building"] {
            assert!(
                style.layers.iter().any(|l| l.source_layer == sl),
                "missing layer for `{sl}`"
            );
        }
    }

    #[test]
    fn width_stops_interpolate_linearly_and_clamp() {
        let w = Width::Stops(vec![(9.0, 0.8), (12.0, 1.6), (15.0, 3.0)]);
        assert_eq!(w.at(9.0), 0.8);
        assert_eq!(w.at(12.0), 1.6);
        assert!((w.at(10.5) - 1.2).abs() < 1e-6, "midpoint of 0.8..1.6");
        assert_eq!(w.at(4.0), 0.8, "clamped below");
        assert_eq!(w.at(20.0), 3.0, "clamped above");
    }

    #[test]
    fn filters_match_native_jsonb_types() {
        let attrs = serde_json::json!({"is_index": true, "class": "sti", "elev_m": 200});
        assert!(Filter::Eq("is_index".into(), serde_json::json!(true)).matches(&attrs));
        assert!(!Filter::Eq("is_index".into(), serde_json::json!("true")).matches(&attrs));
        assert!(Filter::In(
            "class".into(),
            vec![serde_json::json!("sti"), serde_json::json!("vei")]
        )
        .matches(&attrs));
    }
}
