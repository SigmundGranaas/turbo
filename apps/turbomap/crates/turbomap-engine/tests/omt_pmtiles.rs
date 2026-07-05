//! The real-data golden: an **OpenMapTiles-schema** basemap served from a
//! **PMTiles archive**, rendered through the scene engine.
//!
//! This is the first test where the full production data path runs end to
//! end: OMT layer/field taxonomy (`water` / `landuse` / `transportation` /
//! `boundary` / `place`, with `class` + `rank` fields) → MVT encode → a v3
//! `.pmtiles` archive (our writer) → ranged reads (`open_bytes`, the same
//! `RangeReader` contract HTTP uses) → MVT decode → style rules keyed on the
//! OMT schema → pixels. The fixture is generated analytically so tiles agree
//! at boundaries and the archive is deterministic — no binary in the repo.
#![cfg(feature = "gpu-tests")]

use std::sync::Arc;

use turbomap_core::MapOptions;
use turbomap_engine::{
    CameraState, LatLng, MapEngine, ResolvedSource, SourceResolver, TurbomapEngine,
};
use turbomap_golden::{
    assert_golden, headless, render_to_image, sources::ParchmentBasemap, GoldenConfig,
    TARGET_FORMAT,
};
use turbomap_mvt::encode::TileEncoder;
use turbomap_mvt::Value;
use turbomap_scene::{
    Color, Filter, FilterValue, Layer, MatchCase, Paint, Scene, SourceDef, SymbolPlacement,
    TextAnchor,
};
use turbomap_tiles_pmtiles::{writer::write_archive, PMTilesSource, TileType};

const EXTENT: i64 = 4096;
const BUFFER: i64 = 64;
const FIXTURE_ZOOM: u8 = 5;

/// Encode one OMT-schema tile of the analytic world. Same global-lattice
/// technique as `turbomap-sim`'s world: geometry is defined in world space
/// so adjacent tiles agree, but here the layers and fields are the
/// OpenMapTiles taxonomy the production style will target.
fn omt_tile(z: u8, x: u32, y: u32) -> Vec<u8> {
    let span = 1.0 / (1u64 << z) as f64;
    let wx0 = x as f64 * span;
    let wy0 = y as f64 * span;
    let lx = |w: f64| -> i64 { ((w - wx0) / span * EXTENT as f64).round() as i64 };
    let ly = |w: f64| -> i64 { ((w - wy0) / span * EXTENT as f64).round() as i64 };
    let clamp = |v: i64| -> i32 { v.clamp(-BUFFER, EXTENT + BUFFER) as i32 };
    let buf_w = span * BUFFER as f64 / EXTENT as f64;
    let (bx0, bx1) = (wx0 - buf_w, wx0 + span + buf_w);
    let (by0, by1) = (wy0 - buf_w, wy0 + span + buf_w);

    // Cell lattice for lakes/parks/places: 2^-7 world (≈64 px at z5).
    let cell = 2f64.powi(-7);

    // ---- water: lakes on even cells -------------------------------------
    let mut water = TileEncoder::new().layer("water", EXTENT as u32);
    let half = cell * 0.30;
    let i0 = ((bx0 - half) / cell).floor() as i64;
    let i1 = ((bx1 + half) / cell).ceil() as i64;
    let j0 = ((by0 - half) / cell).floor() as i64;
    let j1 = ((by1 + half) / cell).ceil() as i64;
    for i in i0..=i1 {
        for j in j0..=j1 {
            if (i + j).rem_euclid(4) != 0 {
                continue;
            }
            let cx = (i as f64 + 0.5) * cell;
            let cy = (j as f64 + 0.5) * cell;
            if cx + half < bx0 || cx - half > bx1 || cy + half < by0 || cy - half > by1 {
                continue;
            }
            let ring = [
                (clamp(lx(cx - half)), clamp(ly(cy - half))),
                (clamp(lx(cx + half)), clamp(ly(cy - half))),
                (clamp(lx(cx + half)), clamp(ly(cy + half))),
                (clamp(lx(cx - half)), clamp(ly(cy + half))),
            ];
            if ring[0].0 != ring[1].0 && ring[1].1 != ring[2].1 {
                water = water.polygon(&ring, &[("class", Value::String("lake".into()))]);
            }
        }
    }
    let tile = water.finish();

    // ---- landuse: parks on a disjoint cell subset ------------------------
    let mut landuse = tile.layer("landuse", EXTENT as u32);
    for i in i0..=i1 {
        for j in j0..=j1 {
            if (i + j).rem_euclid(4) != 2 || (2 * i + j).rem_euclid(3) != 0 {
                continue;
            }
            let cx = (i as f64 + 0.5) * cell;
            let cy = (j as f64 + 0.5) * cell;
            if cx + half < bx0 || cx - half > bx1 || cy + half < by0 || cy - half > by1 {
                continue;
            }
            let ring = [
                (clamp(lx(cx - half)), clamp(ly(cy - half))),
                (clamp(lx(cx + half)), clamp(ly(cy - half))),
                (clamp(lx(cx + half)), clamp(ly(cy + half))),
                (clamp(lx(cx - half)), clamp(ly(cy + half))),
            ];
            if ring[0].0 != ring[1].0 && ring[1].1 != ring[2].1 {
                landuse = landuse.polygon(&ring, &[("class", Value::String("park".into()))]);
            }
        }
    }
    let tile = landuse.finish();

    // ---- transportation: nested road grid, class per level ---------------
    // motorway 2^-5 (256 px @ z5), primary 2^-6, minor 2^-7.
    let mut roads = tile.layer("transportation", EXTENT as u32);
    let levels: [(&str, i32); 3] = [("motorway", 5), ("primary", 6), ("minor", 7)];
    for (li, &(class, pow)) in levels.iter().enumerate() {
        let spacing = 2f64.powi(-pow);
        let owned_by_coarser = |index: i64| -> bool {
            levels[..li].iter().any(|&(_, cp)| {
                let ratio = 2f64.powi(pow - cp) as i64;
                ratio > 1 && index.rem_euclid(ratio) == 0
            })
        };
        let props = [("class", Value::String(class.into()))];
        let gi0 = (bx0 / spacing).ceil() as i64;
        let gi1 = (bx1 / spacing).floor() as i64;
        for i in gi0..=gi1 {
            if owned_by_coarser(i) {
                continue;
            }
            let wx = i as f64 * spacing;
            roads = roads.line(
                &[
                    (clamp(lx(wx)), -(BUFFER as i32)),
                    (clamp(lx(wx)), (EXTENT + BUFFER) as i32),
                ],
                &props,
            );
        }
        let gj0 = (by0 / spacing).ceil() as i64;
        let gj1 = (by1 / spacing).floor() as i64;
        for j in gj0..=gj1 {
            if owned_by_coarser(j) {
                continue;
            }
            let wy = j as f64 * spacing;
            roads = roads.line(
                &[
                    (-(BUFFER as i32), clamp(ly(wy))),
                    ((EXTENT + BUFFER) as i32, clamp(ly(wy))),
                ],
                &props,
            );
        }
    }
    let tile = roads.finish();

    // ---- boundary: one horizontal admin line per coarse lattice row ------
    let mut boundary = tile.layer("boundary", EXTENT as u32);
    let bspacing = 2f64.powi(-5);
    let gj0 = (by0 / bspacing).ceil() as i64;
    let gj1 = (by1 / bspacing).floor() as i64;
    for j in gj0..=gj1 {
        // Offset off the motorway grid so the dashes read on their own.
        let wy = (j as f64 + 0.43) * bspacing;
        if wy < by0 || wy > by1 {
            continue;
        }
        boundary = boundary.line(
            &[
                (-(BUFFER as i32), clamp(ly(wy))),
                ((EXTENT + BUFFER) as i32, clamp(ly(wy))),
            ],
            &[("admin_level", Value::Int(4))],
        );
    }
    let tile = boundary.finish();

    // ---- place: city points on odd cells ---------------------------------
    const NAMES: [&str; 6] = ["Vik", "Foss", "Nes", "Berg", "Dal", "Strand"];
    let mut places = tile.layer("place", EXTENT as u32);
    let pspacing = 2f64.powi(-6);
    let pi0 = (wx0 / pspacing).ceil() as i64;
    let pi1 = ((wx0 + span) / pspacing).floor() as i64;
    let pj0 = (wy0 / pspacing).ceil() as i64;
    let pj1 = ((wy0 + span) / pspacing).floor() as i64;
    for i in pi0..=pi1 {
        for j in pj0..=pj1 {
            if (i + j).rem_euclid(2) != 1 {
                continue;
            }
            let px = lx(i as f64 * pspacing);
            let py = ly(j as f64 * pspacing);
            if (0..EXTENT).contains(&px) && (0..EXTENT).contains(&py) {
                let name = NAMES[(i * 3 + j).rem_euclid(NAMES.len() as i64) as usize];
                places = places.point(
                    (px as i32, py as i32),
                    &[
                        ("name", Value::String(name.into())),
                        ("class", Value::String("city".into())),
                        ("rank", Value::Int((i + j).rem_euclid(10))),
                    ],
                );
            }
        }
    }
    places.finish().finish()
}

/// Pack the fixture world (a tile ring around Bergen at z5) into a
/// `.pmtiles` archive's bytes.
fn build_fixture_archive() -> Vec<u8> {
    let mut tiles = Vec::new();
    for x in 13..=20u32 {
        for y in 6..=12u32 {
            tiles.push((
                turbomap_core::TileId::new(FIXTURE_ZOOM, x, y),
                omt_tile(FIXTURE_ZOOM, x, y),
            ));
        }
    }
    write_archive(TileType::Mvt, &tiles).expect("fixture archive")
}

/// Resolves the raster base to parchment and the vector source to the
/// PMTiles fixture — the production data path with deterministic bytes.
struct OmtPmtilesResolver {
    archive: Vec<u8>,
}

impl SourceResolver for OmtPmtilesResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(ParchmentBasemap)),
            SourceDef::VectorXyz { .. } => ResolvedSource::Vector(Arc::new(
                PMTilesSource::open_bytes(self.archive.clone()).expect("open fixture archive"),
            )),
            _ => ResolvedSource::Unsupported,
        }
    }
}

/// The OMT-schema style: rules keyed on OpenMapTiles layer names and
/// `class` fields, the way a production style addresses real data.
fn omt_scene() -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "base".to_string(),
        SourceDef::RasterXyz {
            tiles: vec!["https://example.test/{z}/{x}/{y}.png".to_string()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: None,
        },
    );
    scene.sources.insert(
        "omt".to_string(),
        SourceDef::VectorXyz {
            tiles: vec!["pmtiles://fixture".to_string()],
            min_zoom: FIXTURE_ZOOM,
            max_zoom: FIXTURE_ZOOM,
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "water".to_string(),
        source: "omt".to_string(),
        source_layer: Some("water".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(166, 204, 222)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "landuse-park".to_string(),
        source: "omt".to_string(),
        source_layer: Some("landuse".to_string()),
        filter: Filter::Eq("class".to_string(), FilterValue::String("park".to_string())),
        color: Paint::Const(Color::rgb(201, 224, 192)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "boundary".to_string(),
        source: "omt".to_string(),
        source_layer: Some("boundary".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(140, 110, 160)),
        width: Paint::Const(2.0),
        dash_array: Some(vec![9.0, 6.0]),
    });
    scene.layers.push(Layer::Line {
        id: "roads".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: Filter::Always,
        color: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase {
                    value: FilterValue::String("motorway".into()),
                    result: Color::rgb(233, 164, 80),
                },
                MatchCase {
                    value: FilterValue::String("primary".into()),
                    result: Color::rgb(252, 252, 253),
                },
            ],
            default: Box::new(Color::rgb(215, 212, 205)),
        },
        width: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase {
                    value: FilterValue::String("motorway".into()),
                    result: 7.0f32,
                },
                MatchCase {
                    value: FilterValue::String("primary".into()),
                    result: 4.0f32,
                },
            ],
            default: Box::new(2.0f32),
        },
        dash_array: None,
    });
    scene.layers.push(Layer::Symbol {
        id: "place-labels".to_string(),
        source: "omt".to_string(),
        source_layer: Some("place".to_string()),
        filter: Filter::Eq("class".to_string(), FilterValue::String("city".to_string())),
        text_field: "name".to_string(),
        text_size: Paint::Const(15.0),
        color: Paint::Const(Color::rgb(70, 74, 84)),
        halo_color: Paint::Const(Color::rgb(248, 248, 250)),
        halo_width: Paint::Const(1.5),
        sort_key: Some("rank".to_string()),
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
        text_anchor: TextAnchor::Center,
        letter_spacing: 0.0,
        font_weight: 0.0,
    });
    scene
}

/// The offline cold-start gate (plan B6, decisions D2/D7): a Scene that
/// declares its basemap as a `pmtiles-vector` bundle renders COMPLETELY —
/// through the production `HostDrivenResolver`, with zero host fetches and
/// nothing left pending — from one local file. This is the bundled-baseline
/// promise stated as a test: no network, no host IO loop, full map.
#[test]
fn bundled_pmtiles_scene_is_fully_offline_via_the_production_resolver() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    // The bundle: the analytic OMT world packed into a real .pmtiles file.
    let bundle = tempfile::NamedTempFile::new().expect("temp bundle");
    std::fs::write(bundle.path(), build_fixture_archive()).expect("write bundle");

    // The scene names the bundle declaratively — no custom resolver, no
    // URL-template hack. Same style stack as the golden, minus the raster
    // base (a stub source would leave host-pending tiles by design).
    let mut scene = omt_scene();
    scene.sources.insert(
        "omt".to_string(),
        SourceDef::PmtilesVector {
            location: bundle.path().to_string_lossy().into_owned(),
        },
    );
    scene.layers.retain(|l| l.id() != "basemap");
    scene.sources.remove("base");

    let (width, height) = (512, 384);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), f64::from(FIXTURE_ZOOM)),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(turbomap_engine::HostDrivenResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    assert!(
        engine.unsupported_layers().is_empty(),
        "the production resolver must accept pmtiles sources, got {:?}",
        engine.unsupported_layers()
    );

    let stats = engine.pump_tiles();
    assert!(
        stats.vector_tiles >= 4,
        "tiles must be served from the bundle in-process, got {stats:?}"
    );
    // THE offline invariant: after the in-process drain, nothing is left
    // for a host to fetch — a cold start with no network shows a full map.
    let pending = engine.pending_tiles();
    assert!(
        pending.is_empty(),
        "offline scene must leave zero host-pending tiles, got {}",
        pending.len()
    );

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Coverage census — the map is genuinely there, not just "no errors".
    let near =
        |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol);
    let water = image
        .pixels()
        .filter(|p| near(p, [166, 204, 222], 14))
        .count();
    let motorway = image
        .pixels()
        .filter(|p| near(p, [233, 164, 80], 25))
        .count();
    assert!(
        water > 200,
        "bundled water must render offline, got {water}"
    );
    assert!(
        motorway > 200,
        "bundled roads must render offline, got {motorway}"
    );
}

fn pending_zoom(p: &turbomap_core::PendingTile) -> u8 {
    match p {
        turbomap_core::PendingTile::Raster { tile, .. }
        | turbomap_core::PendingTile::Vector { tile, .. }
        | turbomap_core::PendingTile::Hillshade { tile, .. }
        | turbomap_core::PendingTile::Terrain { tile } => tile.z,
    }
}

/// The bundled-under-remote chain gate (plan B6.2): ONE source id backed by
/// `chain [bundle, remote-xyz]`. At the bundle's zoom the map renders fully
/// offline (zero pending). Zoomed past the bundle, the engine surfaces
/// exactly the detail tiles for the host to fetch — graceful refinement,
/// with layers and styles never knowing which provider serves them.
#[test]
fn a_chained_source_renders_offline_and_surfaces_detail_to_the_host() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let bundle = tempfile::NamedTempFile::new().expect("temp bundle");
    std::fs::write(bundle.path(), build_fixture_archive()).expect("write bundle");

    let mut scene = omt_scene();
    scene.sources.insert(
        "omt".to_string(),
        SourceDef::Chain {
            providers: vec![
                SourceDef::PmtilesVector {
                    location: bundle.path().to_string_lossy().into_owned(),
                },
                SourceDef::VectorXyz {
                    tiles: vec!["https://tiles.example/{z}/{x}/{y}.pbf".to_string()],
                    min_zoom: 0,
                    max_zoom: 15,
                },
            ],
        },
    );
    scene.layers.retain(|l| l.id() != "basemap");
    scene.sources.remove("base");

    let (width, height) = (512, 384);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), f64::from(FIXTURE_ZOOM)),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(turbomap_engine::HostDrivenResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    assert!(engine.unsupported_layers().is_empty());

    // At the bundle's zoom the visible view is served fully offline. The
    // chain's zoom union (remote reaches z0) makes the core also want a
    // coarse overview floor below the fixture's single z5 level — tiles the
    // bundle genuinely lacks, correctly surfaced to the host. The invariant
    // is therefore per-tier, not "pending empty": nothing AT the visible
    // zoom may pend. (A production baseline bundles the coarse zooms too,
    // which is exactly what the pure-bundle test above proves.)
    let stats = engine.pump_tiles();
    assert!(
        stats.vector_tiles >= 4,
        "bundle must serve the coarse view, got {stats:?}"
    );
    let unserved_visible: Vec<_> = engine
        .pending_tiles()
        .iter()
        .filter(|p| pending_zoom(p) == FIXTURE_ZOOM)
        .cloned()
        .collect();
    assert!(
        unserved_visible.is_empty(),
        "every visible-zoom tile must come from the bundle, got {unserved_visible:?}"
    );
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();
    let near =
        |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol);
    let water = image
        .pixels()
        .filter(|p| near(p, [166, 204, 222], 14))
        .count();
    assert!(
        water > 200,
        "chained source must render the bundle offline, got {water}"
    );

    // Past the bundle: the same source surfaces detail tiles as pending —
    // the host's fetch signal, exactly as if the chain weren't there.
    engine.set_camera(CameraState::new(
        LatLng::new(60.39, 5.32),
        f64::from(FIXTURE_ZOOM) + 2.0,
    ));
    let _ = engine.pump_tiles(); // stub providers make no progress in-process
    let pending = engine.pending_tiles();
    assert!(
        !pending.is_empty(),
        "detail zoom must surface pending tiles for the host to fetch"
    );
}

#[test]
fn omt_schema_renders_from_a_pmtiles_archive() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let (width, height) = (512, 384);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), f64::from(FIXTURE_ZOOM)),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(OmtPmtilesResolver {
            archive: build_fixture_archive(),
        }),
    )
    .expect("construct TurbomapEngine");

    engine.apply(omt_scene());
    let stats = engine.pump_tiles();
    assert!(
        stats.vector_tiles >= 4,
        "several tiles should be served from the archive, got {stats:?}"
    );
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let near =
        |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol);
    let water = image
        .pixels()
        .filter(|p| near(p, [166, 204, 222], 14))
        .count();
    let park = image
        .pixels()
        .filter(|p| near(p, [201, 224, 192], 14))
        .count();
    let motorway = image
        .pixels()
        .filter(|p| near(p, [233, 164, 80], 25))
        .count();
    let boundary = image
        .pixels()
        .filter(|p| near(p, [140, 110, 160], 30))
        .count();
    let ink = image.pixels().filter(|p| near(p, [70, 74, 84], 30)).count();
    eprintln!("omt: water={water} park={park} motorway={motorway} boundary={boundary} ink={ink}");
    assert!(water > 200, "OMT water lakes should render, got {water}");
    assert!(park > 150, "OMT landuse parks should render, got {park}");
    assert!(
        motorway > 200,
        "OMT motorway class should render, got {motorway}"
    );
    assert!(
        boundary > 60,
        "dashed admin boundary should render, got {boundary}"
    );
    assert!(ink > 40, "OMT place labels should render, got {ink}");

    assert_golden(
        "omt-pmtiles-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
