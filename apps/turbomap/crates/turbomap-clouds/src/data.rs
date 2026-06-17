//! Radar data model and a synthetic weather generator.
//!
//! # Data contract
//!
//! The cloud renderer consumes a small, low-resolution grid — exactly the
//! shape of the "blocky" rasters MET Norway publishes:
//!
//! - **Radar reflectivity** (`api.met.no/weatherapi/radar/2.0`) and the
//!   gridded **nowcast** product give per-cell *precipitation intensity*.
//! - **Locationforecast** / the AROME grid give per-cell
//!   *cloud_area_fraction* (total cloud cover).
//!
//! Each [`RadarFrame`] cell therefore carries two channels in `0..=1`:
//!
//! - [`Cell::precip`] — rain intensity. Drives how *dark* the cloud paints
//!   (light cumulus → charcoal storm).
//! - [`Cell::coverage`] — cloud-area fraction. Drives *where* and how much
//!   cloud exists at all.
//!
//! A live integration replaces [`SyntheticStorm`] with a fetch+decode step
//! that fills the same grid: sample the radar/cloud raster at each cell's
//! lat/lon (Web-Mercator tile space), normalise to `0..=1`, and hand the
//! frame to [`crate::CloudScene::upload`]. Everything downstream — the GPU
//! pipeline, the time crossfade — is source-agnostic.

/// One radar grid cell: rain intensity and cloud-cover fraction, `0..=1`.
#[derive(Copy, Clone, Debug, Default)]
pub struct Cell {
    /// Precipitation intensity, normalised `0..=1`. `0` = dry.
    pub precip: f32,
    /// Cloud-area fraction, normalised `0..=1`. `0` = clear sky.
    pub coverage: f32,
}

/// A single timestep of the radar grid: a `width × height` field of cells
/// in row-major order, plus the wall-clock instant it represents.
#[derive(Clone, Debug)]
pub struct RadarFrame {
    pub width: u32,
    pub height: u32,
    /// Minutes since the sequence start — what a time slider scrubs over.
    pub minutes: f32,
    pub cells: Vec<Cell>,
}

impl RadarFrame {
    fn new(width: u32, height: u32, minutes: f32) -> Self {
        Self {
            width,
            height,
            minutes,
            cells: vec![Cell::default(); (width * height) as usize],
        }
    }

    /// Build a frame from two `width * height` byte planes — the shape a
    /// host hands across an FFI boundary after sampling MET radar/nowcast
    /// (precip) and cloud-cover (coverage) rasters and normalising each to
    /// `0..=255`. Lengths shorter than `width * height` are zero-padded;
    /// extra bytes are ignored.
    pub fn from_u8(width: u32, height: u32, precip: &[u8], coverage: &[u8]) -> Self {
        let n = (width * height) as usize;
        let cells = (0..n)
            .map(|i| Cell {
                precip: precip.get(i).copied().unwrap_or(0) as f32 / 255.0,
                coverage: coverage.get(i).copied().unwrap_or(0) as f32 / 255.0,
            })
            .collect();
        Self {
            width,
            height,
            minutes: 0.0,
            cells,
        }
    }

    /// Pack into the `Rgba8Unorm` layout the shader samples: `R` =
    /// precip, `G` = coverage. `B`/`A` are reserved (kept at 255) so the
    /// texture stays renderable/inspectable as an ordinary image.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.cells.len() * 4);
        for c in &self.cells {
            out.push((c.precip.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.push((c.coverage.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.push(0);
            out.push(255);
        }
        out
    }
}

/// Parameters for the synthetic storm used by the demo. Tuned to look like
/// a Norwegian autumn day: a frontal rain band sweeping in from the west
/// over broken cloud, with a couple of intense convective cores embedded
/// in it.
pub struct SyntheticStorm {
    pub width: u32,
    pub height: u32,
    /// Number of timesteps to generate.
    pub frames: usize,
    /// Simulated minutes between consecutive frames (MET radar is 5-min).
    pub minutes_per_frame: f32,
}

impl Default for SyntheticStorm {
    fn default() -> Self {
        // ~64×42 keeps cells visibly "blocky" at screen size — the same
        // coarse look as the source radar we are prettifying.
        Self {
            width: 64,
            height: 42,
            frames: 12,
            minutes_per_frame: 5.0,
        }
    }
}

impl SyntheticStorm {
    /// Generate the full sequence. Cells are deliberately quantised so the
    /// raw input reads as a blocky radar raster — the GPU pass is what
    /// turns it into smooth cloud.
    pub fn generate(&self) -> Vec<RadarFrame> {
        let w = self.width as f32;
        let h = self.height as f32;
        (0..self.frames)
            .map(|fi| {
                let t = fi as f32 / (self.frames.max(2) - 1) as f32; // 0..1
                let mut frame =
                    RadarFrame::new(self.width, self.height, fi as f32 * self.minutes_per_frame);

                // Frontal band: a soft diagonal line of rain sweeping
                // west→east across the grid as `t` advances.
                let front_x = -0.2 + t * 1.35; // normalised x of the band

                // A few convective cores riding the front, each drifting
                // and pulsing on its own phase. (phase, y, x0, amp)
                let cores = [
                    (0.10f32, 0.38f32, 0.95f32, 0.85f32),
                    (-0.05, 0.60, 0.18, 0.6),
                    (0.22, 0.74, 0.55, 0.55),
                ];

                // Large, dry-ish fair-weather cloud masses with clear sky
                // between them — drifting east. (y, x0, sigma, amp)
                let masses = [
                    (0.30f32, 0.78f32, 0.16f32, 0.85f32),
                    (0.66, 0.30, 0.20, 0.95),
                    (0.50, 0.05, 0.13, 0.7),
                    (0.18, 0.50, 0.12, 0.6),
                    (0.82, 0.62, 0.15, 0.75),
                ];

                for y in 0..self.height {
                    for x in 0..self.width {
                        let nx = (x as f32 + 0.5) / w; // 0..1
                        let ny = (y as f32 + 0.5) / h;

                        // Distance to the slanted frontal line.
                        let band_pos = nx + (ny - 0.5) * 0.3;
                        let band = gauss(band_pos - front_x, 0.10);

                        // Coverage: the rainy frontal cloud, plus the dry
                        // fair-weather masses, over an essentially clear
                        // sky (small wandering ambient, not a uniform
                        // deck — that was what produced the "rash").
                        let mut coverage = band * 0.85 + ambient(nx, ny, t);
                        // Precip lives only in the wettest heart of the
                        // band; the fair-weather masses stay dry.
                        let mut precip = band * band * 0.55;

                        for (cy, cx0, sigma, amp) in masses {
                            let cx = cx0 + t * 0.9 + 0.03 * (t * 4.0 + cy * 9.0).sin();
                            let cyy = cy + 0.04 * (t * 3.0 + cx0 * 7.0).cos();
                            let d = ((nx - cx).powi(2) + (ny - cyy).powi(2)).sqrt();
                            coverage += gauss(d, sigma) * amp;
                        }

                        for (phase, cy, cx0, amp) in cores {
                            // Each core drifts east with the front and
                            // bobs north/south slightly.
                            let cx = cx0 + t * 1.2 + 0.04 * (t * 6.0 + phase).sin();
                            let cyy = cy + 0.05 * (t * 5.0 + phase * 3.0).cos();
                            let d = ((nx - cx).powi(2) + (ny - cyy).powi(2)).sqrt();
                            // Pulse the intensity over time so cores grow
                            // and decay rather than just translate.
                            let pulse =
                                0.55 + 0.45 * (t * 7.0 + phase * std::f32::consts::TAU).sin();
                            let core = gauss(d, 0.06) * amp * pulse;
                            precip += core;
                            coverage += core * 0.9;
                        }

                        // Quantise to ~12 levels so the raw frame looks
                        // like a discrete radar product.
                        let q = |v: f32| (v.clamp(0.0, 1.0) * 12.0).round() / 12.0;
                        let idx = (y * self.width + x) as usize;
                        frame.cells[idx] = Cell {
                            precip: q(precip),
                            coverage: q(coverage),
                        };
                    }
                }
                frame
            })
            .collect()
    }
}

/// Unnormalised gaussian falloff.
fn gauss(d: f32, sigma: f32) -> f32 {
    (-(d * d) / (2.0 * sigma * sigma)).exp()
}

/// Faint, slowly wandering wisps so the "clear" sky isn't dead flat —
/// stays low (≈0..0.15) so it never fills in as a uniform overcast deck.
fn ambient(x: f32, y: f32, t: f32) -> f32 {
    let a = (x * 4.0 + t * 1.2).sin() * (y * 3.5 - t * 0.8).cos();
    (0.06 + 0.09 * a).clamp(0.0, 0.18)
}
