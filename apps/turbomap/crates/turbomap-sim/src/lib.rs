//! Headless device-equivalent for the turbomap engine.
//!
//! "Test it like a phone would run it, without the phone": scripted user
//! sessions drive the real engine frame by frame — camera animations,
//! tiles arriving with simulated network latency, every frame rendered
//! and measured — so Google-Maps-class *behaviour* (no blank map while
//! zooming, frames converge after motion, labels/roads/water all paint,
//! interaction stays correct) is asserted in CI, not eyeballed on a
//! device.
//!
//! - [`world`] — a deterministic synthetic city served as real MVT bytes
//!   (grid road hierarchy, lakes, named places), so density changes with
//!   zoom like a real basemap and the byte-level ingest path is the one
//!   production uses.
//! - [`session`] — the per-frame driver + pixel instrumentation.
//! - [`perf`] — percentile summaries over recorded frames; the profiling
//!   surface (CPU now; wall-clock and GPU timestamps as lanes mature).
//!
//! What this cannot replace — said plainly: absolute performance on
//! mobile GPUs (this runs on a software rasteriser; numbers are
//! *relative* regression signals) and platform surface lifecycle, which
//! stays in the per-platform glue.

pub mod perf;
pub mod session;
pub mod world;

pub use perf::PerfSummary;
pub use session::{basemap_scene, diff_fraction, fraction_near, FrameStats, Sim};
