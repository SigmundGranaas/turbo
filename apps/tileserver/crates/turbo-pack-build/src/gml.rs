//! Streaming GML reader for the two shapes this pipeline needs:
//! `<app:område>` surfaces and `<app:senterlinje>` curves.
//!
//! Streaming rather than DOM because the N50 Arealdekke file for a
//! single kommune is 95 MB uncompressed, and the whole point of this
//! crate is that it runs somewhere a 95 MB parse tree is not welcome.
//! Coordinates land straight in EPSG:25833 — the pack's own frame — so
//! nothing here reprojects.
//!
//! # Element shape
//!
//! ```xml
//! <app:Innsjø gml:id="...">
//!   <app:område>
//!     <gml:Surface srsName="urn:ogc:def:crs:EPSG::25833">
//!       <gml:patches><gml:PolygonPatch>
//!         <gml:exterior><gml:LinearRing><gml:posList>x y x y …
//!         <gml:interior>…
//! ```
//!
//! Curves (`Veglenke`, and the WFS's `traktorveg_sti`) carry
//! `<gml:LineString><gml:posList>` or a `<gml:Curve>` with
//! `<gml:segments><gml:LineStringSegment>`. Both are handled, because
//! N50 and the WFS do not agree on which they emit.

use geo::{Coord, LineString, Polygon};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::BuildError;

/// Strip any namespace prefix: `app:Innsjø` → `Innsjø`.
fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.rfind(':') {
        Some(i) => s[i + 1..].to_string(),
        None => s.to_string(),
    }
}

/// `"x y x y …"` → coordinates.
///
/// `srsDimension` may be 3 on some N50 layers, in which case the
/// triplets carry a Z that is dropped. Deciding by *parity* rather than
/// by trusting the attribute: a mis-declared dimension would otherwise
/// shear every ring into nonsense that still parses.
fn parse_pos_list(text: &str, dims: usize) -> Vec<Coord<f64>> {
    let nums: Vec<f64> = text
        .split_ascii_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    let step = if dims >= 3 && nums.len() % 3 == 0 {
        3
    } else {
        2
    };
    nums.chunks(step)
        .filter(|c| c.len() >= 2)
        .map(|c| Coord { x: c[0], y: c[1] })
        .collect()
}

/// Close a ring if the source left it open. An unclosed exterior makes
/// the scanline fill leak along the missing segment.
fn close(mut ring: Vec<Coord<f64>>) -> Vec<Coord<f64>> {
    if ring.len() >= 3 && ring[0] != ring[ring.len() - 1] {
        ring.push(ring[0]);
    }
    ring
}

/// Feed every `<app:område>` polygon of the named feature types to `sink`.
///
/// Multi-patch surfaces yield one polygon per patch, which matches what
/// `ST_Dump` gives the PostGIS builder.
pub fn read_surfaces(
    xml: &str,
    want: &[&str],
    mut sink: impl FnMut(&str, Polygon<f64>),
) -> Result<usize, BuildError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut feature: Option<String> = None;
    let mut depth_in_feature = 0usize;
    let mut in_area = false;
    let mut in_exterior = false;
    let mut in_interior = false;
    let mut in_pos = false;
    let mut dims = 2usize;
    let mut exterior: Vec<Coord<f64>> = Vec::new();
    let mut interiors: Vec<Vec<Coord<f64>>> = Vec::new();
    let mut count = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(BuildError::Decode(format!("GML: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = local(e.name().as_ref());
                if feature.is_none() {
                    if want.iter().any(|w| *w == name) {
                        feature = Some(name);
                        depth_in_feature = 0;
                    }
                    buf.clear();
                    continue;
                }
                depth_in_feature += 1;
                match name.as_str() {
                    "område" => in_area = true,
                    "exterior" if in_area => in_exterior = true,
                    "interior" if in_area => in_interior = true,
                    "posList" if in_area => {
                        in_pos = true;
                        dims = e
                            .attributes()
                            .flatten()
                            .find(|a| local(a.key.as_ref()) == "srsDimension")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse().ok())
                            .unwrap_or(2);
                    }
                    // A surface may hold several patches; each is its own
                    // polygon, so flush the one in hand before starting
                    // the next.
                    "PolygonPatch" | "Polygon" if in_area && !exterior.is_empty() => {
                        if let Some(f) = &feature {
                            sink(f, build(&mut exterior, &mut interiors));
                            count += 1;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_pos => {
                let text = t
                    .unescape()
                    .map_err(|e| BuildError::Decode(e.to_string()))?;
                let coords = parse_pos_list(&text, dims);
                if in_exterior {
                    exterior = close(coords);
                } else if in_interior {
                    interiors.push(close(coords));
                }
            }
            Ok(Event::End(e)) => {
                let name = local(e.name().as_ref());
                match name.as_str() {
                    "posList" => in_pos = false,
                    "exterior" => in_exterior = false,
                    "interior" => in_interior = false,
                    "område" => in_area = false,
                    _ => {}
                }
                if feature.as_deref() == Some(name.as_str()) && depth_in_feature == 0 {
                    if !exterior.is_empty() {
                        if let Some(f) = &feature {
                            sink(f, build(&mut exterior, &mut interiors));
                            count += 1;
                        }
                    }
                    exterior.clear();
                    interiors.clear();
                    feature = None;
                } else if feature.is_some() {
                    depth_in_feature = depth_in_feature.saturating_sub(1);
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(count)
}

fn build(exterior: &mut Vec<Coord<f64>>, interiors: &mut Vec<Vec<Coord<f64>>>) -> Polygon<f64> {
    let ext = LineString::from(std::mem::take(exterior));
    let ints = std::mem::take(interiors)
        .into_iter()
        .map(LineString::from)
        .collect();
    Polygon::new(ext, ints)
}

/// One line feature: its type name and its vertices in EPSG:25833.
#[derive(Debug, Clone)]
pub struct LineFeature {
    pub kind: String,
    pub coords: Vec<Coord<f64>>,
    /// Raw child-element text, for the attributes the graph cares about
    /// (`typeVeg`, `rutemerking`, …).
    pub attrs: Vec<(String, String)>,
}

/// Feed every curve of the named feature types to `sink`.
pub fn read_lines(
    xml: &str,
    want: &[&str],
    mut sink: impl FnMut(LineFeature),
) -> Result<usize, BuildError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut feature: Option<String> = None;
    let mut in_pos = false;
    let mut dims = 2usize;
    let mut coords: Vec<Coord<f64>> = Vec::new();
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut attr_key: Option<String> = None;
    let mut count = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(BuildError::Decode(format!("GML: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = local(e.name().as_ref());
                if feature.is_none() {
                    if want.iter().any(|w| *w == name) {
                        feature = Some(name);
                        coords.clear();
                        attrs.clear();
                    }
                    buf.clear();
                    continue;
                }
                match name.as_str() {
                    "posList" | "pos" => {
                        in_pos = true;
                        dims = e
                            .attributes()
                            .flatten()
                            .find(|a| local(a.key.as_ref()) == "srsDimension")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse().ok())
                            .unwrap_or(2);
                    }
                    // Anything else with text is a candidate attribute.
                    other => attr_key = Some(other.to_string()),
                }
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map_err(|e| BuildError::Decode(e.to_string()))?;
                if in_pos {
                    // A Curve's segments each carry a posList; they are
                    // one line, so append rather than replace, dropping
                    // the duplicated joint vertex.
                    let mut c = parse_pos_list(&text, dims);
                    if !coords.is_empty() && coords.last() == c.first() {
                        c.remove(0);
                    }
                    coords.append(&mut c);
                } else if let Some(k) = attr_key.take() {
                    if feature.is_some() && !text.trim().is_empty() {
                        attrs.push((k, text.trim().to_string()));
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local(e.name().as_ref());
                if name == "posList" || name == "pos" {
                    in_pos = false;
                }
                if feature.as_deref() == Some(name.as_str()) {
                    if coords.len() >= 2 {
                        sink(LineFeature {
                            kind: name.clone(),
                            coords: std::mem::take(&mut coords),
                            attrs: std::mem::take(&mut attrs),
                        });
                        count += 1;
                    }
                    coords.clear();
                    attrs.clear();
                    feature = None;
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped exactly like the real N50 Arealdekke, down to the
    /// Surface/patches/PolygonPatch nesting ArcGIS emits.
    const LAKE: &str = r#"
<gml:featureMember xmlns:gml="g" xmlns:app="a">
  <app:Innsjø gml:id="id1">
    <app:oppdateringsdato>2016-12-02</app:oppdateringsdato>
    <app:område>
      <gml:Surface srsName="urn:ogc:def:crs:EPSG::25833" srsDimension="2">
        <gml:patches><gml:PolygonPatch>
          <gml:exterior><gml:LinearRing>
            <gml:posList>0 0 100 0 100 100 0 100 0 0</gml:posList>
          </gml:LinearRing></gml:exterior>
          <gml:interior><gml:LinearRing>
            <gml:posList>40 40 60 40 60 60 40 60 40 40</gml:posList>
          </gml:LinearRing></gml:interior>
        </gml:PolygonPatch></gml:patches>
      </gml:Surface>
    </app:område>
    <app:høyde>481</app:høyde>
  </app:Innsjø>
</gml:featureMember>"#;

    #[test]
    fn reads_a_surface_with_a_hole() {
        let mut got = Vec::new();
        let n = read_surfaces(LAKE, &["Innsjø"], |k, p| got.push((k.to_string(), p))).unwrap();
        assert_eq!(n, 1);
        assert_eq!(got[0].0, "Innsjø");
        assert_eq!(got[0].1.exterior().0.len(), 5);
        assert_eq!(got[0].1.interiors().len(), 1, "the island was dropped");
    }

    /// Feature types not asked for must not leak in — Arealdekke holds
    /// Skog and Myr in the same file, and flooding the mask with forest
    /// would refuse most of Norway.
    #[test]
    fn ignores_feature_types_not_requested() {
        let n = read_surfaces(LAKE, &["SnøIsbre"], |_, _| panic!("should not emit")).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn drops_the_z_from_three_dimensional_pos_lists() {
        let xml = LAKE
            .replace("srsDimension=\"2\"", "srsDimension=\"3\"")
            .replace(
            "<gml:posList>0 0 100 0 100 100 0 100 0 0</gml:posList>",
            "<gml:posList srsDimension=\"3\">0 0 5 100 0 5 100 100 5 0 100 5 0 0 5</gml:posList>",
        );
        let mut got = None;
        read_surfaces(&xml, &["Innsjø"], |_, p| got = Some(p)).unwrap();
        let ext = got.expect("polygon").exterior().0.clone();
        assert_eq!(ext.len(), 5);
        assert_eq!(ext[1], Coord { x: 100.0, y: 0.0 }, "z leaked into x/y");
    }

    #[test]
    fn closes_an_open_ring() {
        let xml = LAKE.replace(
            "<gml:posList>0 0 100 0 100 100 0 100 0 0</gml:posList>",
            "<gml:posList>0 0 100 0 100 100 0 100</gml:posList>",
        );
        let mut got = None;
        read_surfaces(&xml, &["Innsjø"], |_, p| got = Some(p)).unwrap();
        let ext = got.unwrap().exterior().0.clone();
        assert_eq!(
            ext.first(),
            ext.last(),
            "ring left open — the fill would leak"
        );
    }

    const ROAD: &str = r#"
<gml:featureMember xmlns:gml="g" xmlns:app="a">
  <app:Veglenke gml:id="v1">
    <app:typeVeg>enkelBilveg</app:typeVeg>
    <app:senterlinje>
      <gml:Curve srsName="urn:ogc:def:crs:EPSG::25833"><gml:segments>
        <gml:LineStringSegment><gml:posList>0 0 10 10</gml:posList></gml:LineStringSegment>
        <gml:LineStringSegment><gml:posList>10 10 20 30</gml:posList></gml:LineStringSegment>
      </gml:segments></gml:Curve>
    </app:senterlinje>
  </app:Veglenke>
</gml:featureMember>"#;

    /// A multi-segment Curve is one line, and the shared joint vertex
    /// must not be duplicated — a repeated vertex is a zero-length
    /// stretch that shows up later as a degenerate edge.
    #[test]
    fn joins_curve_segments_without_duplicating_the_joint() {
        let mut got = Vec::new();
        let n = read_lines(ROAD, &["Veglenke"], |f| got.push(f)).unwrap();
        assert_eq!(n, 1);
        let f = &got[0];
        assert_eq!(
            f.coords,
            vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 10.0, y: 10.0 },
                Coord { x: 20.0, y: 30.0 },
            ]
        );
        assert!(f
            .attrs
            .iter()
            .any(|(k, v)| k == "typeVeg" && v == "enkelBilveg"));
    }

    #[test]
    fn skips_a_line_with_a_single_vertex() {
        let xml = ROAD.replace(
            "<gml:posList>0 0 10 10</gml:posList>",
            "<gml:posList>0 0</gml:posList>",
        ).replace(
            "<gml:LineStringSegment><gml:posList>10 10 20 30</gml:posList></gml:LineStringSegment>",
            "",
        );
        let n = read_lines(&xml, &["Veglenke"], |_| panic!("should not emit")).unwrap();
        assert_eq!(n, 0);
    }
}
