//! `/v1/packs/:key/:file`, driven as a client drives it.
//!
//! Against the real committed pack (`tools/ci-pack`) used as a *source*
//! artifact set, so the slice runs for real rather than against a
//! fixture that cannot fail the way production data does.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use turbo_tiles_api::packs::PackService;

/// The committed pack, standing in for the national artifacts.
fn source_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ci-pack")
        .canonicalize()
        .expect("tools/ci-pack is committed")
}

/// A key well inside the CI pack's coverage — [14.94, 67.03] ..
/// [15.12, 67.10], which is Sjunkhatten.
///
/// Verified by `a_built_pack_actually_contains_terrain` below, and that
/// test exists because the first version of this file used a key 50 km
/// east. Every assertion still passed: slicing a region with no source
/// tiles produces a perfectly valid pack containing nothing, and "the
/// files exist" cannot tell that from a pack of a real valley.
const KEY: &str = "z12_2218_1007_2219_1008";

fn service(cache: &std::path::Path) -> PackService {
    PackService::new(source_dir(), cache.to_path_buf(), 1)
}

#[tokio::test]
async fn a_pack_builds_on_first_request_and_is_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(tmp.path());
    let key = svc.parse_key(KEY).expect("the key must parse");

    // First touch builds. `pack.toml` is the trigger on purpose: it is a
    // few hundred bytes, so paying the build there reads as "preparing"
    // rather than as a stalled multi-megabyte download.
    let manifest = svc.file(&key, "pack.toml").await.expect("must build");
    assert!(manifest.is_file());

    // The artifacts are now on disk beside it.
    for f in ["norway.dem", "norway.mask", "norway.graph"] {
        assert!(
            svc.file(&key, f).await.unwrap().is_file(),
            "{f} must exist after the build"
        );
    }

    // Second call must not rebuild. Checked by mtime rather than by
    // timing, which would be flaky on a loaded machine.
    let before = std::fs::metadata(&manifest).unwrap().modified().unwrap();
    let again = svc.file(&key, "pack.toml").await.unwrap();
    assert_eq!(
        std::fs::metadata(&again).unwrap().modified().unwrap(),
        before,
        "a built pack must be served, not rebuilt"
    );
}

#[tokio::test]
async fn a_built_pack_actually_contains_terrain() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(tmp.path());
    let key = svc.parse_key(KEY).unwrap();
    let dem = svc.file(&key, "norway.dem").await.unwrap();

    // An empty slice is a valid pack. It has a header, a manifest, a
    // digest that verifies — and no ground. The only thing that
    // distinguishes it from a real one is size, so size is what this
    // asserts: a z12 pair at this latitude is ~4 x 8 km of 10 m DEM,
    // which cannot fit in a few kilobytes of header.
    let bytes = std::fs::metadata(&dem).unwrap().len();
    assert!(
        bytes > 200_000,
        "norway.dem is {bytes} B — this key is cutting empty ground, so \
         every other test in this file is passing on nothing. Check the \
         key against the CI pack's extent."
    );

    // And the pack must route, which is the property that actually
    // matters and the one no size check can stand in for.
    let engine =
        turbo_route_ffi::RouteEngine::open(dem.parent().unwrap().to_string_lossy().into_owned())
            .expect("a served pack must open");
    let cov = engine.coverage();
    assert!(cov.max_lat > cov.min_lat && cov.max_lon > cov.min_lon);
}

#[tokio::test]
async fn the_manifest_lists_every_file_with_a_matching_digest() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(tmp.path());
    let key = svc.parse_key(KEY).unwrap();
    let path = svc.file(&key, "pack.toml").await.unwrap();

    let m: turbo_tiles_artifacts::PackManifest =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(m.check_compatible().is_ok());
    assert!(!m.pack.files.is_empty(), "the manifest must list its files");
    assert!(m.total_bytes() > 0);

    // The digests are the only end-to-end integrity check between an
    // artifact on the origin and the bytes a phone routes on. If they do
    // not match what was written, a client that verifies will reject
    // every pack and one that does not will trust a corrupt one.
    for f in &m.pack.files {
        let bytes = std::fs::read(path.parent().unwrap().join(&f.name)).unwrap();
        assert_eq!(bytes.len() as u64, f.bytes, "{} size", f.name);
        let digest: String = {
            use sha2::{Digest, Sha256};
            Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        };
        assert_eq!(digest, f.sha256, "{} digest", f.name);
    }
}

#[tokio::test]
async fn simultaneous_requests_build_once() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = std::sync::Arc::new(service(tmp.path()));
    let key = svc.parse_key(KEY).unwrap();

    // Eight clients asking for the same region at once is not a corner
    // case — it is one user's download, with five files, retried. Without
    // the per-key lock each one starts its own slice of the national
    // artifacts and the server falls over doing the same work eight times.
    let mut set = Vec::new();
    for _ in 0..8 {
        let svc = svc.clone();
        set.push(tokio::spawn(async move {
            svc.file(&key, "norway.dem").await.map(|p| p.is_file())
        }));
    }
    for h in set {
        assert!(h.await.unwrap().unwrap());
    }

    // One directory, one build. A duplicate build would have raced
    // through the same rename and left the loser's temp dir behind.
    let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("partial"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "leftover partial builds: {leftovers:?}"
    );
}

#[tokio::test]
async fn a_key_bigger_than_the_cap_is_refused_before_any_work() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(tmp.path());
    // ~2500 cells. Without a cap this endpoint is a denial-of-service
    // primitive with a friendly URL: one GET, arbitrary CPU.
    let err = svc.parse_key("z12_1000_1000_1049_1049").unwrap_err();
    assert!(
        err.to_string().contains("too large"),
        "expected a size refusal, got {err}"
    );
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "a refused request must not have started a build"
    );
}

#[tokio::test]
async fn a_file_name_cannot_escape_the_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(tmp.path());
    let key = svc.parse_key(KEY).unwrap();
    for bad in ["../pack.toml", "..", "a/b", "", "norway.dem/../../x"] {
        assert!(
            svc.file(&key, bad).await.is_err(),
            "{bad:?} must not resolve"
        );
    }
}

#[tokio::test]
async fn the_router_serves_a_pack_file_and_says_it_is_immutable() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = turbo_tiles_api::ApiState::for_tests();
    state.packs = Some(std::sync::Arc::new(service(tmp.path())));
    let app = turbo_tiles_api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/packs/{KEY}/pack.toml"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    // Immutable per key is the whole reason this can sit behind a CDN:
    // same key, same bytes, forever. A rebuild bumps the edge's data
    // version rather than changing this response.
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );
    assert!(resp.headers().get("content-length").is_some());
}

#[tokio::test]
async fn a_malformed_key_is_a_400_not_a_500() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = turbo_tiles_api::ApiState::for_tests();
    state.packs = Some(std::sync::Arc::new(service(tmp.path())));
    let app = turbo_tiles_api::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/packs/not-a-key/pack.toml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // The client's mistake, and retrying will not help — distinguishable
    // from "this region has no data", which is what a 404 would say.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
