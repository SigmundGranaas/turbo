//! Quantitative fidelity metrics for the cloud overlay.
//!
//! "Looks like spilled milk" is a visual complaint; to *fix* it we need
//! numbers that say **how faithfully the rendered overlay represents the
//! radar data it was given**. This module is pure CPU math (no GPU, no
//! `image` dependency) so it can be unit-tested and run as a regression
//! gate. The GPU harness renders the diagnostic AOVs (alpha, lit colour),
//! down-samples them to the radar grid with [`box_downsample`], and hands
//! the grids here.
//!
//! The two failure modes we measure, mapped to what the eye sees:
//!
//! * **Silhouette** — does cloud sit *where the data says*? `leak` (cloud
//!   over empty sky → the milk spreading everywhere), `miss` (no cloud
//!   over solid overcast), `silhouette_iou`, and the coverage→alpha
//!   correlation all score this.
//! * **Intensity** — is heavy rain rendered *dark*? `precip_darkness_corr`
//!   scores whether opacity-weighted darkness tracks precipitation.

/// One fidelity scorecard for a rendered frame vs. its radar input.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Fidelity {
    /// Pearson correlation between per-cell coverage and rendered alpha.
    /// `1` = the silhouette tracks the data perfectly. Higher is better.
    pub coverage_alpha_corr: f32,
    /// Mean rendered alpha over cells with essentially no coverage. This is
    /// the "spilled milk" number: cloud painted where the radar says clear
    /// sky. Lower is better (`0` = no leak).
    pub leak: f32,
    /// `1 −` mean rendered alpha over solidly-covered cells: overcast that
    /// failed to render as cloud. Lower is better (`0` = full coverage).
    pub miss: f32,
    /// Intersection-over-union of the coverage mask and the alpha mask
    /// (both thresholded at `0.3`). Higher is better.
    pub silhouette_iou: f32,
    /// Correlation between precipitation and rendered darkness
    /// (`1 − luminance`), measured only over cells that actually have
    /// cloud. Positive = heavier rain reads darker. Higher is better.
    pub precip_darkness_corr: f32,
    /// Fraction of cells that rendered as cloud (alpha > `0.3`). Context for
    /// the other numbers — a metric over 3 cells is noise.
    pub clouded_fraction: f32,
}

/// Decode an 8-bit sRGB sample (`0..=255`) to a linear `0..=1` value. The
/// diagnostic AOVs are written through an sRGB render target, so a scalar
/// the shader emitted as `gray(v)` comes back sRGB-encoded; this recovers
/// the original linear `v`.
pub fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.040_448_237 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Box-average one channel of an RGBA pixel buffer down to a `gw × gh`
/// grid, sRGB-decoding each sample to linear. Returns `gw * gh` values in
/// `0..=1`, row-major — aligned with [`crate::RadarFrame::cells`].
///
/// Panics if `pixels` is not `width * height * 4` bytes.
pub fn box_downsample(
    pixels: &[u8],
    width: u32,
    height: u32,
    gw: u32,
    gh: u32,
    channel: usize,
) -> Vec<f32> {
    assert_eq!(
        pixels.len(),
        (width * height * 4) as usize,
        "pixel buffer must be width*height*4 RGBA bytes"
    );
    assert!(channel < 4, "channel must be 0..=3");
    let mut out = vec![0.0f32; (gw * gh) as usize];
    for gy in 0..gh {
        for gx in 0..gw {
            // Pixel span this grid cell covers.
            let x0 = (gx * width / gw) as usize;
            let x1 = ((gx + 1) * width / gw).max(gx * width / gw + 1) as usize;
            let y0 = (gy * height / gh) as usize;
            let y1 = ((gy + 1) * height / gh).max(gy * height / gh + 1) as usize;
            let mut sum = 0.0f32;
            let mut n = 0u32;
            for py in y0..y1.min(height as usize) {
                for px in x0..x1.min(width as usize) {
                    let i = (py * width as usize + px) * 4 + channel;
                    sum += srgb_to_linear(pixels[i]);
                    n += 1;
                }
            }
            out[(gy * gw + gx) as usize] = if n > 0 { sum / n as f32 } else { 0.0 };
        }
    }
    out
}

/// Pearson correlation of two equal-length series. Returns `0` for a
/// degenerate (constant) series rather than `NaN`, so it is safe to feed
/// to a regression threshold.
pub fn pearson(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "series must be equal length");
    let n = a.len();
    if n == 0 {
        return 0.0;
    }
    let inv = 1.0 / n as f32;
    let ma = a.iter().sum::<f32>() * inv;
    let mb = b.iter().sum::<f32>() * inv;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for i in 0..n {
        let da = a[i] - ma;
        let db = b[i] - mb;
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    let denom = (va * vb).sqrt();
    if denom <= f32::EPSILON {
        0.0
    } else {
        cov / denom
    }
}

/// Relative luminance (Rec. 709) of a linear RGB triplet.
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Score a rendered frame against its radar input.
///
/// * `coverage` / `precip` — the radar grid channels, `gw * gh` each.
/// * `alpha` — the rendered alpha AOV, down-sampled to the grid.
/// * `luma` — per-cell luminance of the lit-albedo AOV, down-sampled.
///
/// All four slices must be `gw * gh` long.
pub fn evaluate(coverage: &[f32], precip: &[f32], alpha: &[f32], luma: &[f32]) -> Fidelity {
    let n = coverage.len();
    assert!(
        precip.len() == n && alpha.len() == n && luma.len() == n,
        "all grids must be the same length"
    );

    const CLEAR: f32 = 0.05; // coverage below this = clear sky
    const SOLID: f32 = 0.60; // coverage above this = solid overcast
    const ON: f32 = 0.30; // alpha/coverage threshold for the masks

    let coverage_alpha_corr = pearson(coverage, alpha);

    let leak = mean_where(alpha, |i| coverage[i] < CLEAR).unwrap_or(0.0);
    let fill = mean_where(alpha, |i| coverage[i] > SOLID).unwrap_or(1.0);
    let miss = (1.0 - fill).clamp(0.0, 1.0);

    let silhouette_iou = iou(coverage, alpha, ON);

    // Darkness ↔ rain only makes sense where there's cloud to be dark.
    let clouded: Vec<usize> = (0..n).filter(|&i| alpha[i] > ON).collect();
    let clouded_fraction = clouded.len() as f32 / n.max(1) as f32;
    let precip_darkness_corr = if clouded.len() >= 2 {
        let p: Vec<f32> = clouded.iter().map(|&i| precip[i]).collect();
        let dark: Vec<f32> = clouded.iter().map(|&i| 1.0 - luma[i]).collect();
        pearson(&p, &dark)
    } else {
        0.0
    };

    Fidelity {
        coverage_alpha_corr,
        leak,
        miss,
        silhouette_iou,
        precip_darkness_corr,
        clouded_fraction,
    }
}

/// Mean of `xs` over the indices where `pred` holds; `None` if none do.
fn mean_where(xs: &[f32], pred: impl Fn(usize) -> bool) -> Option<f32> {
    let mut sum = 0.0;
    let mut n = 0u32;
    for (i, &x) in xs.iter().enumerate() {
        if pred(i) {
            sum += x;
            n += 1;
        }
    }
    (n > 0).then(|| sum / n as f32)
}

/// IoU of `{a > t}` and `{b > t}` over matching indices.
fn iou(a: &[f32], b: &[f32], t: f32) -> f32 {
    let mut inter = 0u32;
    let mut union = 0u32;
    for i in 0..a.len() {
        let ina = a[i] > t;
        let inb = b[i] > t;
        if ina || inb {
            union += 1;
        }
        if ina && inb {
            inter += 1;
        }
    }
    if union == 0 {
        1.0 // both empty: trivially perfect agreement
    } else {
        inter as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearson_is_one_for_a_perfect_line() {
        let a = [0.0, 0.25, 0.5, 0.75, 1.0];
        let b = [0.0, 0.5, 1.0, 1.5, 2.0]; // b = 2a
        assert!((pearson(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pearson_is_minus_one_when_inverted_and_zero_when_constant() {
        let a = [0.0, 0.5, 1.0];
        let inv = [1.0, 0.5, 0.0];
        assert!((pearson(&a, &inv) + 1.0).abs() < 1e-5);
        let flat = [0.4, 0.4, 0.4];
        assert_eq!(pearson(&a, &flat), 0.0); // no NaN on a constant series
    }

    #[test]
    fn srgb_decode_hits_known_anchors() {
        assert_eq!(srgb_to_linear(0), 0.0);
        assert!((srgb_to_linear(255) - 1.0).abs() < 1e-5);
        // Mid grey 188/255 ≈ 0.5 linear (the classic sRGB midpoint).
        assert!((srgb_to_linear(188) - 0.5).abs() < 0.02);
    }

    #[test]
    fn box_downsample_averages_blocks() {
        // 2x2 image, one channel: top row 255, bottom row 0 → into a 1x2
        // grid each cell is a pure row (255→1.0 linear, 0→0.0).
        let px = vec![
            255, 0, 0, 255, 255, 0, 0, 255, // row 0 (two white-R pixels)
            0, 0, 0, 255, 0, 0, 0, 255, // row 1 (two black-R pixels)
        ];
        let g = box_downsample(&px, 2, 2, 1, 2, 0);
        assert!((g[0] - 1.0).abs() < 1e-4); // top cell
        assert!(g[1].abs() < 1e-4); // bottom cell
    }

    #[test]
    fn faithful_render_scores_well() {
        // alpha == coverage, darkness == precip → ideal fidelity.
        let coverage = [0.0, 0.1, 0.9, 1.0, 0.0, 0.8];
        let precip = [0.0, 0.0, 0.5, 0.9, 0.0, 0.2];
        let alpha = coverage;
        // luma falls as precip rises where cloud exists.
        let luma: Vec<f32> = precip.iter().map(|&p| 1.0 - p).collect();
        let f = evaluate(&coverage, &precip, &alpha, &luma);
        assert!(f.coverage_alpha_corr > 0.95, "{f:?}");
        assert!(f.leak < 0.01, "{f:?}");
        assert!(f.miss < 0.2, "{f:?}");
        assert!(f.silhouette_iou > 0.9, "{f:?}");
        assert!(f.precip_darkness_corr > 0.95, "{f:?}");
    }

    #[test]
    fn spilled_milk_render_is_caught() {
        // Uniform cloud everywhere regardless of the data: max leak, the
        // silhouette doesn't track coverage, IoU collapses.
        let coverage = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0];
        let precip = [0.0, 0.0, 0.0, 0.8, 0.8, 0.0];
        let alpha = [0.9, 0.9, 0.9, 0.9, 0.9, 0.9];
        let luma = [0.9, 0.9, 0.9, 0.9, 0.9, 0.9]; // uniformly bright, ignores rain
        let f = evaluate(&coverage, &precip, &alpha, &luma);
        assert!(f.leak > 0.8, "leak should be high: {f:?}");
        assert!(f.coverage_alpha_corr.abs() < 0.2, "no correlation: {f:?}");
        assert!(f.silhouette_iou < 0.5, "iou should collapse: {f:?}");
    }
}
