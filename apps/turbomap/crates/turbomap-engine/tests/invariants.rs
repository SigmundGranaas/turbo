//! Mechanical enforcement of the architecture's grep-enforceable invariants
//! (architecture doc Part VI; full-adoption plan P6.0). These ran as manual
//! greps through Phases 0–5 — this test makes reintroducing a violation fail
//! `cargo test`, not a code review.
//!
//! THE RATCHET RULE: every allowlist below may only SHRINK. Adding a file to
//! an allowlist is an architecture decision that needs a written entry in the
//! plan doc's progress log, not a convenience.
//!
//! No GPU, no features — this runs in the plain workspace lane.

use std::fs;
use std::path::{Path, PathBuf};

/// `apps/turbomap` — the Rust workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

/// Every `.rs` file under `crates/`, skipping build output.
fn rust_sources() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![workspace_root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable source dir") {
            let path = entry.expect("dir entry").path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if path.is_dir() {
                if name != "target" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") && name != "invariants.rs" {
                // This file necessarily spells the forbidden patterns.
                let text = fs::read_to_string(&path).expect("readable source file");
                out.push((path, text));
            }
        }
    }
    assert!(
        out.len() > 50,
        "source walk looks broken — found only {} .rs files",
        out.len()
    );
    out
}

/// Path relative to the workspace root, with `/` separators, for stable
/// allowlists and readable failure messages.
fn rel(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .expect("source under workspace")
        .to_string_lossy()
        .replace('\\', "/")
}

/// Lines containing `needle`, excluding comment lines — invariants bind code,
/// not the doc comments that explain their history.
fn code_hits<'a>(text: &'a str, needle: &str) -> Vec<(usize, &'a str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim_start();
            line.contains(needle) && !t.starts_with("//") && !t.starts_with("*")
        })
        .map(|(i, line)| (i + 1, line))
        .collect()
}

fn assert_no_hits(files: &[(PathBuf, String)], needle: &str, allowed: &[&str], law: &str) {
    let mut violations = Vec::new();
    for (path, text) in files {
        let rp = rel(path);
        if allowed.iter().any(|a| rp == *a) {
            continue;
        }
        for (line, src) in code_hits(text, needle) {
            violations.push(format!("  {rp}:{line}: {}", src.trim()));
        }
    }
    assert!(
        violations.is_empty(),
        "INVARIANT VIOLATED — {law}\npattern `{needle}` outside its allowlist:\n{}",
        violations.join("\n")
    );
}

/// P5.1's gate, permanent: the streaming plan is the only tile transport.
/// `pending_tiles` as a *call* must not exist — hosts consume
/// `streaming_plan` + `report_fetch_*`; the engine's synchronous local pump
/// goes through the `#[doc(hidden)]` preview, not a revived pull API.
#[test]
fn one_tile_transport_no_pending_tiles_calls() {
    assert_no_hits(
        &rust_sources(),
        ".pending_tiles(",
        &[],
        "one transport (plan P5.1): no pull-push revival",
    );
}

/// Invariant 10, image half: wire-format decoding happens at the codec (or on
/// the transport side of it) — never downstream of the interpretation plane.
/// `turbomap-core`, `turbomap-scene`, and `turbomap-world` must stay
/// format-blind; tests/examples decode render *output* for assertions, which
/// is not the tile pipeline.
#[test]
fn formats_die_at_the_codec_image() {
    let allowed = [
        // The codec — the one interpretation plane.
        "crates/turbomap-engine/src/codec.rs",
        // The engine's synchronous local pump (goldens/tests fast path).
        // TODO(P6.2): fold `fetch_decode` into the codec and delete this line.
        "crates/turbomap-engine/src/engine.rs",
        // Golden harness decodes rendered PNGs to compare them — output side,
        // not tile ingestion.
        "crates/turbomap-golden/src/trace.rs",
    ];
    let files: Vec<_> = rust_sources()
        .into_iter()
        .filter(|(p, _)| {
            let rp = rel(p);
            !rp.contains("/tests/") && !rp.contains("/examples/")
        })
        .collect();
    assert_no_hits(
        &files,
        "image::load_from_memory",
        &allowed,
        "no format past the codec (invariant 10)",
    );
}

/// Invariant 10, vector half: MVT wire parsing is confined to the codec and
/// the in-process tile-source providers (transport side). Core re-exports the
/// decoded *types* — the FeatureSet representation — but never parses bytes.
#[test]
fn formats_die_at_the_codec_mvt() {
    let allowed = [
        "crates/turbomap-engine/src/codec.rs",
        // In-process sources implement request_decoded for the local pump —
        // transport side of the interpretation plane.
        "crates/turbomap-tiles-http/src/lib.rs",
        "crates/turbomap-tiles-pmtiles/src/lib.rs",
        // The sim's synthetic world round-trips its own encoded tiles.
        "crates/turbomap-sim/src/world.rs",
    ];
    let files: Vec<_> = rust_sources()
        .into_iter()
        .filter(|(p, _)| {
            let rp = rel(p);
            !rp.contains("/tests/") && !rp.contains("/examples/")
        })
        .collect();
    assert_no_hits(
        &files,
        "turbomap_mvt::decode(",
        &allowed,
        "no format past the codec (invariant 10)",
    );
}

/// D3's gate, permanent: DEM wire encodings never reach the render path.
/// Heights upload as real floats; `DemEncoding` may appear in the IR, the
/// codec, and source plumbing — never under `core/src/render/` or in WGSL.
#[test]
fn dem_encoding_stays_out_of_the_render_path() {
    let mut violations = Vec::new();
    for (path, text) in rust_sources() {
        let rp = rel(&path);
        if !rp.starts_with("crates/turbomap-core/src/render/") {
            continue;
        }
        for (line, src) in code_hits(&text, "DemEncoding") {
            violations.push(format!("  {rp}:{line}: {}", src.trim()));
        }
    }
    // WGSL: the shader must not branch on wire encodings either.
    let mut stack = vec![workspace_root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable dir") {
            let path = entry.expect("dir entry").path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if path.is_dir() {
                if name != "target" {
                    stack.push(path);
                }
            } else if name.ends_with(".wgsl") {
                let text = fs::read_to_string(&path).expect("readable wgsl");
                if text.contains("decode_elevation") || text.contains("dem_encoding") {
                    violations.push(format!("  {}: wire decode in shader", rel(&path)));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "INVARIANT VIOLATED — DEM decode out of the render path (D3):\n{}",
        violations.join("\n")
    );
}

/// The elevation-decode formula has exactly one home (D3): `core/src/dem.rs`.
#[test]
fn decode_elevation_has_one_home() {
    assert_no_hits(
        &rust_sources(),
        "fn decode_elevation",
        &["crates/turbomap-core/src/dem.rs"],
        "one home for the DEM formula (D3)",
    );
}

/// Invariant 7, total (P5.2 + P6.1): content has exactly ONE authoring
/// surface — the Scene IR. No imperative content setter may exist on the
/// engine or its bindings, hidden or public. Core `Map` keeps these methods
/// as `reconcile`'s private write path; nothing above it may.
#[test]
fn content_has_one_authoring_surface() {
    const SETTERS: &[&str] = &[
        "fn set_sun_position",
        "fn set_terrain_shadows",
        "fn set_clouds_visible",
        "fn set_route_tube",
        // The P6.5 slot verbs are core `Map` reconcile plumbing, same rule.
        "fn add_tube_layer",
        "fn add_circle_layer",
        "fn add_marker_to_layer",
        "fn enable_clouds",
        "fn disable_clouds",
        "fn set_cloud_sim",
        "fn set_cloud_geo_bounds",
        "fn set_sun_time",
        "fn set_basemap_gain",
        "fn set_terrain_lit",
        "fn set_aerial_haze",
    ];
    let files: Vec<_> = rust_sources()
        .into_iter()
        .filter(|(p, _)| {
            let rp = rel(p);
            rp.starts_with("crates/turbomap-engine/src/")
                || rp.starts_with("crates/turbomap-ffi/src/")
                || rp.starts_with("crates/turbomap-web/src/")
        })
        .collect();
    for needle in SETTERS {
        assert_no_hits(
            &files,
            needle,
            &[],
            "one authoring surface for content (invariant 7, P6.1)",
        );
    }
}

/// P6.2's gate, permanent: the engine owns the core `Map`. The desktop app
/// was the last host holding a `&mut Map` — it is a Scene host now, so no
/// crate outside `turbomap-engine` may reach the wrapped map. (The engine's
/// own GPU tests may: `map_mut` stays as the in-crate `#[doc(hidden)]`
/// debug/test hook.) No allowlist — the count is zero and stays zero.
#[test]
fn engine_owns_the_map_no_map_mut_outside() {
    let files: Vec<_> = rust_sources()
        .into_iter()
        .filter(|(p, _)| {
            let rp = rel(p);
            !rp.starts_with("crates/turbomap-engine/")
                && !rp.contains("/tests/")
                && !rp.contains("/examples/")
        })
        .collect();
    assert_no_hits(
        &files,
        ".map_mut(",
        &[],
        "the engine owns the map (P6.2): hosts author Scenes, not Map calls",
    );
}
