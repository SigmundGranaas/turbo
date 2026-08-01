//! Parse the real N50 GML, not a synthetic fixture.
//!
//! The synthetic tests in `gml.rs` pin the shapes this parser is
//! *designed* for. This one answers the different question of whether
//! the file Kartverket actually ships is one of those shapes — the
//! nesting ArcGIS emits, the Norwegian element names, the 95 MB of it.
//!
//! Skipped unless the caller supplies an extracted Arealdekke file:
//!
//! ```text
//! TURBO_N50_AREALDEKKE=/path/Arealdekke.gml \
//! TURBO_N50_SAMFERDSEL=/path/Samferdsel.gml \
//!   cargo test -p turbo-pack-build --test real_gml -- --nocapture
//! ```

use std::collections::BTreeMap;

use turbo_pack_build::gml;

/// What `upsert_n50_vann.sql` and `upsert_n50_isogbre.sql` select.
const WATER: &[&str] = &["Innsjø", "InnsjøRegulert", "Elv", "Havflate"];
const GLACIER: &[&str] = &["SnøIsbre"];

#[test]
fn parses_the_real_arealdekke_surfaces() {
    let Ok(path) = std::env::var("TURBO_N50_AREALDEKKE") else {
        eprintln!("skipped: set TURBO_N50_AREALDEKKE");
        return;
    };
    let xml = std::fs::read_to_string(&path).expect("read Arealdekke");

    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut rings_with_holes = 0usize;
    let mut degenerate = 0usize;
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    let want: Vec<&str> = WATER.iter().chain(GLACIER.iter()).copied().collect();
    let n = gml::read_surfaces(&xml, &want, |kind, poly| {
        *by_kind.entry(kind.to_string()).or_default() += 1;
        if !poly.interiors().is_empty() {
            rings_with_holes += 1;
        }
        let ext = &poly.exterior().0;
        if ext.len() < 4 {
            degenerate += 1;
        }
        for c in ext {
            min_x = min_x.min(c.x);
            max_x = max_x.max(c.x);
            min_y = min_y.min(c.y);
            max_y = max_y.max(c.y);
        }
    })
    .expect("parse");

    eprintln!("{n} surfaces {by_kind:?}");
    eprintln!(
        "  {rings_with_holes} with holes, {degenerate} degenerate, \
         bbox ({min_x:.0}, {min_y:.0}) - ({max_x:.0}, {max_y:.0})"
    );

    // Counts from an independent scan of the same file, so this catches
    // the parser silently dropping a feature type.
    assert_eq!(by_kind.get("Innsjø").copied().unwrap_or(0), 6657);
    assert_eq!(by_kind.get("Havflate").copied().unwrap_or(0), 86);
    assert_eq!(by_kind.get("SnøIsbre").copied().unwrap_or(0), 52);
    assert_eq!(by_kind.get("Elv").copied().unwrap_or(0), 85);
    assert_eq!(by_kind.get("InnsjøRegulert").copied().unwrap_or(0), 11);

    assert_eq!(
        degenerate, 0,
        "rings with fewer than 4 vertices cannot be filled"
    );
    assert!(
        rings_with_holes > 0,
        "no interior rings at all — islands are being dropped"
    );

    // Sørfold in EPSG:25833. A parser that swapped x and y, or leaked a
    // Z, lands far outside this and is caught here rather than as a
    // mask full of ocean.
    assert!(
        (400_000.0..700_000.0).contains(&min_x) && (400_000.0..700_000.0).contains(&max_x),
        "eastings {min_x:.0}..{max_x:.0} are not UTM33 for Sørfold"
    );
    assert!(
        (7_300_000.0..7_600_000.0).contains(&min_y) && (7_300_000.0..7_600_000.0).contains(&max_y),
        "northings {min_y:.0}..{max_y:.0} are not northern Norway"
    );
}

#[test]
fn parses_the_real_samferdsel_lines() {
    let Ok(path) = std::env::var("TURBO_N50_SAMFERDSEL") else {
        eprintln!("skipped: set TURBO_N50_SAMFERDSEL");
        return;
    };
    let xml = std::fs::read_to_string(&path).expect("read Samferdsel");

    let mut vertices = 0usize;
    let mut with_type = 0usize;
    let mut shortest = f64::MAX;
    let n = gml::read_lines(&xml, &["Veglenke"], |f| {
        vertices += f.coords.len();
        if f.attrs.iter().any(|(k, _)| k == "typeVeg") {
            with_type += 1;
        }
        let mut len = 0.0;
        for w in f.coords.windows(2) {
            len += ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        }
        shortest = shortest.min(len);
    })
    .expect("parse");

    eprintln!(
        "{n} Veglenke, {vertices} vertices, {with_type} with typeVeg, shortest {shortest:.1} m"
    );
    assert_eq!(n, 1341);
    assert!(
        vertices > n * 2,
        "lines averaging under 2 vertices is a parse failure"
    );
    assert_eq!(
        with_type, n,
        "typeVeg is what the graph classifies roads by"
    );
}
