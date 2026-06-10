//! Headless golden-image + record/replay test harness for turbomap.
//!
//! This crate exists so that *every render path* can be exercised
//! deterministically in CI without a window, a network, or a real GPU:
//!
//! - [`gpu`] — a headless wgpu context (prefers a software adapter) and a
//!   single-frame offscreen readback.
//! - [`sources`] — deterministic, in-process synthetic tile sources.
//! - [`trace`] — a serialisable `Trace` describing a scene, plus a
//!   `replay` runner that turns one into a final composite image.
//! - [`golden`] — perceptual comparison against committed reference PNGs,
//!   regenerated with `UPDATE_GOLDEN=1`.
//!
//! The harness library itself has no `#[test]`s — those live in
//! `tests/golden.rs` behind the `gpu-tests` feature, so the default
//! `cargo test --workspace` lane (no GPU) compiles this crate but runs
//! nothing, while the dedicated golden CI lane runs the full suite.

pub mod golden;
pub mod gpu;
pub mod omt;
pub mod sources;
pub mod trace;

pub use golden::{assert_golden, GoldenConfig};
pub use gpu::{headless, render_to_image, Gpu, TARGET_FORMAT};
pub use trace::{replay, CameraSpec, LayerSpec, SourceSpec, Trace};
