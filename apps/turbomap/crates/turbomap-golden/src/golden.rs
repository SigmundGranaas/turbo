//! Perceptual golden-image comparison.
//!
//! Software rasterisers are deterministic for a *fixed* Mesa version but
//! drift slightly across versions, so we never compare bit-exactly:
//! a pixel "differs" only if a channel moves by more than
//! `max_channel_diff`, and the image fails only if more than
//! `max_outlier_frac` of pixels differ. References are regenerated with
//! `UPDATE_GOLDEN=1`.

use std::path::PathBuf;

use image::RgbaImage;

/// Tolerance for a single golden comparison.
#[derive(Debug, Clone, Copy)]
pub struct GoldenConfig {
    /// Per-channel absolute difference a pixel may have before it counts
    /// as an outlier. Absorbs rounding between driver versions.
    pub max_channel_diff: u8,
    /// Fraction of total pixels allowed to be outliers before failing.
    pub max_outlier_frac: f64,
}

impl Default for GoldenConfig {
    fn default() -> Self {
        // Tight enough to catch real regressions, loose enough to ride
        // out llvmpipe version drift between a dev box and CI.
        Self {
            max_channel_diff: 4,
            max_outlier_frac: 0.01,
        }
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn failure_dir() -> PathBuf {
    // apps/turbomap/target/golden-failures — a stable path the CI golden
    // lane uploads as an artifact when a comparison fails.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("golden-failures")
}

fn update_requested() -> bool {
    matches!(
        std::env::var("UPDATE_GOLDEN").ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

/// Compare `actual` against the committed reference `tests/golden/<name>.png`.
///
/// - `UPDATE_GOLDEN=1` writes/overwrites the reference and returns.
/// - A missing reference is a hard error pointing at `UPDATE_GOLDEN=1`.
/// - On mismatch, writes `<name>.actual.png` and `<name>.diff.png` to the
///   failure dir and panics with outlier stats.
pub fn assert_golden(name: &str, actual: &RgbaImage, cfg: GoldenConfig) {
    let ref_path = golden_dir().join(format!("{name}.png"));

    if update_requested() {
        std::fs::create_dir_all(golden_dir()).expect("create golden dir");
        actual.save(&ref_path).expect("write golden reference");
        eprintln!("UPDATE_GOLDEN: wrote {}", ref_path.display());
        return;
    }

    let expected = match image::open(&ref_path) {
        Ok(img) => img.to_rgba8(),
        Err(_) => panic!(
            "missing golden reference {}. Generate it with: \
             UPDATE_GOLDEN=1 cargo test -p turbomap-golden --features gpu-tests",
            ref_path.display()
        ),
    };

    assert_eq!(
        (expected.width(), expected.height()),
        (actual.width(), actual.height()),
        "golden '{name}' size mismatch: expected {}x{}, got {}x{}",
        expected.width(),
        expected.height(),
        actual.width(),
        actual.height(),
    );

    let total = (actual.width() * actual.height()) as f64;
    let mut outliers = 0u64;
    let mut max_seen = 0u8;
    let mut diff = RgbaImage::new(actual.width(), actual.height());
    for (a, e, d) in itertools_zip(actual, &expected, &mut diff) {
        let mut pixel_max = 0u8;
        for c in 0..4 {
            pixel_max = pixel_max.max(a.0[c].abs_diff(e.0[c]));
        }
        max_seen = max_seen.max(pixel_max);
        if pixel_max > cfg.max_channel_diff {
            outliers += 1;
            *d = image::Rgba([255, 0, 255, 255]); // magenta marks a diff
        } else {
            *d = image::Rgba([0, 0, 0, 255]);
        }
    }

    let frac = outliers as f64 / total;
    if frac > cfg.max_outlier_frac {
        let dir = failure_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = actual.save(dir.join(format!("{name}.actual.png")));
        let _ = diff.save(dir.join(format!("{name}.diff.png")));
        panic!(
            "golden '{name}' mismatch: {outliers}/{total:.0} pixels ({:.3}%) exceed \
             channel diff {} (max seen {max_seen}); budget {:.3}%. \
             Wrote actual+diff to {}",
            frac * 100.0,
            cfg.max_channel_diff,
            cfg.max_outlier_frac * 100.0,
            dir.display(),
        );
    }
}

/// Tiny three-way pixel zip so we don't pull in the `itertools` crate.
fn itertools_zip<'a>(
    a: &'a RgbaImage,
    e: &'a RgbaImage,
    d: &'a mut RgbaImage,
) -> impl Iterator<
    Item = (
        &'a image::Rgba<u8>,
        &'a image::Rgba<u8>,
        &'a mut image::Rgba<u8>,
    ),
> {
    a.pixels()
        .zip(e.pixels())
        .zip(d.pixels_mut())
        .map(|((a, e), d)| (a, e, d))
}
