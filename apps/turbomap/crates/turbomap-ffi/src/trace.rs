//! Structured per-frame trace — the Slice-1 instrumentation that makes the tile
//! pipeline *measurable* (timings, tile-state histogram, cache health, jitter)
//! instead of guessed at. Published in the snapshot's `stats_json` for the host,
//! and emitted in the same field-set by the offline harness (`scenario.rs`) as
//! CSV, so the device and the harness speak ONE schema (see
//! `docs/architecture/2026-06-turbomap-tile-pipeline-plan.md`).
//!
//! This module is intentionally **ungated** (the `surface` FFI that consumes it
//! is Android-only) so the pure [`FrameTrace::to_json`] serialization is
//! host-compiled and unit-tested in CI — that test is the Slice-1 schema gate.
//! On non-Android host builds these are reached only from tests, hence
//! `allow(dead_code)`.
#![allow(dead_code)]

use turbomap_engine::TurbomapEngine;

/// One frame's trace. Three field groups: per-stage timings, the tile-state
/// histogram + draw load, and cache health. The streaming fields (`gap_ms`,
/// `ingest_ms`, `render_ms`, `ingested`, `backlog`, `pending`) are only
/// meaningful live; the harness drains tiles synchronously and leaves them zero.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct FrameTrace {
    pub frame: u64,
    // Timings (ms).
    pub gap_ms: f64,
    pub cpu_ms: f64,
    pub prepare_ms: f64,
    pub pass_ms: f64,
    pub clouds_ms: f64,
    pub gpu_ms: Option<f64>,
    pub ingest_ms: f64,
    pub render_ms: f64,
    // Tile-state histogram + draw load.
    pub visible_layers: usize,
    pub draw_calls: usize,
    pub tiles_drawn: usize,
    pub resident: usize,
    pub pending: u32,
    pub backlog: usize,
    pub ingested: u32,
    pub frame_dropped: bool,
    // Lifecycle histogram (engine-side truth, summed across layers + terrain):
    // desired = engine-pending + resident; retained = eviction candidates;
    // engine-pending split by tier. `pending` above stays the FFI-live fetch
    // queue; `pend_*` here is the engine's want-list — comparing the two is
    // exactly how starvation vs transport-lag is told apart.
    pub desired: usize,
    pub retained: usize,
    pub pend_overview: usize,
    pub pend_visible: usize,
    pub pend_prefetch: usize,
    // Cache health.
    pub bytes: usize,
    pub budget: usize,
    pub evictions: u64,
    pub hits: u64,
    pub misses: u64,
}

impl FrameTrace {
    /// Compact, flat JSON. Keys are a SUPERSET of the legacy `stats_json`
    /// (`tiles`/`bytes`/`budget`/`evictions`/`hits`/`misses` retained, with
    /// `tiles` mirroring `resident`) so existing host parsers keep working.
    pub fn to_json(self) -> String {
        let gpu = self
            .gpu_ms
            .map(|g| format!("{g:.3}"))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"frame\":{},\"gap_ms\":{:.2},\"cpu_ms\":{:.3},\"prepare_ms\":{:.3},\
             \"pass_ms\":{:.3},\"clouds_ms\":{:.3},\"gpu_ms\":{gpu},\"ingest_ms\":{:.3},\
             \"render_ms\":{:.3},\"visible_layers\":{},\"draw_calls\":{},\"tiles_drawn\":{},\
             \"resident\":{},\"tiles\":{},\"pending\":{},\"backlog\":{},\"ingested\":{},\
             \"desired\":{},\"retained\":{},\"pend_overview\":{},\"pend_visible\":{},\
             \"pend_prefetch\":{},\
             \"frame_dropped\":{},\"bytes\":{},\"budget\":{},\"evictions\":{},\"hits\":{},\"misses\":{}}}",
            self.frame, self.gap_ms, self.cpu_ms, self.prepare_ms, self.pass_ms, self.clouds_ms,
            self.ingest_ms, self.render_ms, self.visible_layers, self.draw_calls, self.tiles_drawn,
            self.resident, self.resident, self.pending, self.backlog, self.ingested,
            self.desired, self.retained, self.pend_overview, self.pend_visible,
            self.pend_prefetch,
            self.frame_dropped, self.bytes, self.budget, self.evictions, self.hits, self.misses,
        )
    }
}

/// Fold the engine's always-on `FrameMetrics` (timings, draw load, cache stats)
/// into one trace. `live` carries the FFI-only streaming timings/counts the
/// engine can't know (gap, ingest budget, in-flight backlog).
pub(crate) fn frame_trace(engine: &TurbomapEngine, live: FrameTrace) -> FrameTrace {
    let m = engine.last_frame_metrics();
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    FrameTrace {
        cpu_ms: ms(m.cpu_time),
        prepare_ms: ms(m.phases.prepare),
        pass_ms: ms(m.phases.pass),
        clouds_ms: ms(m.phases.clouds),
        gpu_ms: m.gpu_time.map(ms),
        visible_layers: m.visible_layers,
        draw_calls: m.draw_calls,
        tiles_drawn: m.tiles_drawn,
        resident: m.layers.iter().map(|l| l.cache.entries).sum(),
        frame_dropped: m.frame_dropped,
        desired: m.tiles.desired,
        retained: m.tiles.retained,
        pend_overview: m.tiles.pending_overview,
        pend_visible: m.tiles.pending_visible,
        pend_prefetch: m.tiles.pending_prefetch,
        bytes: m.layers.iter().map(|l| l.cache.bytes_used).sum(),
        budget: m
            .layers
            .iter()
            .map(|l| l.cache.budget_bytes)
            .max()
            .unwrap_or(0),
        evictions: m.layers.iter().map(|l| l.cache.evictions).sum(),
        hits: m.layers.iter().map(|l| l.cache.hits).sum(),
        misses: m.layers.iter().map(|l| l.cache.misses).sum(),
        ..live
    }
}

/// Cache-only trace for the initial/default snapshot, before any frame has the
/// live streaming timings.
pub(crate) fn stats_json(engine: &TurbomapEngine) -> String {
    frame_trace(engine, FrameTrace::default()).to_json()
}

/// The frame graph's pass report as a JSON array — the decomposition of
/// `pass_ms` (slice D1): one entry per pass instance, in execution order,
/// with its phase, CPU encode time and skip state.
pub(crate) fn passes_json(engine: &TurbomapEngine) -> String {
    passes_json_from(&engine.last_frame_metrics().passes)
}

/// Pure formatter behind [`passes_json`], host-compiled and unit-tested (the
/// engine variant needs a GPU device).
pub(crate) fn passes_json_from(passes: &[turbomap_core::PassTiming]) -> String {
    let items: Vec<String> = passes
        .iter()
        .map(|p| {
            format!(
                "{{\"pass\":\"{}\",\"phase\":\"{:?}\",\"cpu_ms\":{:.3},\"skipped\":{}}}",
                p.label,
                p.phase,
                p.cpu.as_secs_f64() * 1000.0,
                p.skipped,
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trace serializes every documented field, keeps the legacy
    /// `stats_json` keys (with `tiles` mirroring `resident`) for host-parser
    /// back-compat, and renders a missing GPU timestamp as JSON `null` — not a
    /// bogus number. This is the Slice-1 schema gate: harness CSV + device trace
    /// both depend on these exact keys.
    #[test]
    fn frame_trace_json_has_all_fields_and_legacy_keys() {
        let t = FrameTrace {
            frame: 7,
            gap_ms: 18.25,
            cpu_ms: 4.7,
            prepare_ms: 3.3,
            pass_ms: 0.2,
            clouds_ms: 0.0,
            gpu_ms: None,
            ingest_ms: 6.0,
            render_ms: 4.9,
            visible_layers: 2,
            draw_calls: 7,
            tiles_drawn: 440,
            resident: 512,
            pending: 31,
            backlog: 12,
            ingested: 5,
            frame_dropped: false,
            desired: 471,
            retained: 41,
            pend_overview: 1,
            pend_visible: 20,
            pend_prefetch: 10,
            bytes: 1024,
            budget: 2048,
            evictions: 9,
            hits: 100,
            misses: 20,
        };
        let j = t.to_json();
        for key in [
            "\"frame\":7",
            "\"gap_ms\":18.25",
            "\"prepare_ms\":3.300",
            "\"pass_ms\":0.200",
            "\"draw_calls\":7",
            "\"tiles_drawn\":440",
            "\"pending\":31",
            "\"backlog\":12",
            "\"ingested\":5",
            "\"frame_dropped\":false",
            "\"gpu_ms\":null",
            // lifecycle histogram (A1): engine want-list + eviction candidates.
            "\"desired\":471",
            "\"retained\":41",
            "\"pend_overview\":1",
            "\"pend_visible\":20",
            "\"pend_prefetch\":10",
            // legacy keys preserved; `tiles` mirrors `resident`.
            "\"resident\":512",
            "\"tiles\":512",
            "\"bytes\":1024",
            "\"budget\":2048",
            "\"evictions\":9",
            "\"hits\":100",
            "\"misses\":20",
        ] {
            assert!(j.contains(key), "trace JSON missing {key}: {j}");
        }
    }

    #[test]
    fn frame_trace_json_renders_gpu_time_when_present() {
        let t = FrameTrace {
            gpu_ms: Some(0.875),
            ..FrameTrace::default()
        };
        assert!(t.to_json().contains("\"gpu_ms\":0.875"), "{}", t.to_json());
    }

    #[test]
    fn passes_json_serializes_the_pass_report_in_order() {
        use std::time::Duration;
        use turbomap_core::{FramePhase, PassTiming};
        let passes = vec![
            PassTiming {
                label: "sky".to_string(),
                phase: FramePhase::GroundMsaa,
                cpu: Duration::from_micros(120),
                skipped: false,
            },
            PassTiming {
                label: "layer:hillshade".to_string(),
                phase: FramePhase::GroundMsaa,
                cpu: Duration::ZERO,
                skipped: true,
            },
        ];
        let j = passes_json_from(&passes);
        assert_eq!(
            j,
            "[{\"pass\":\"sky\",\"phase\":\"GroundMsaa\",\"cpu_ms\":0.120,\"skipped\":false},\
             {\"pass\":\"layer:hillshade\",\"phase\":\"GroundMsaa\",\"cpu_ms\":0.000,\"skipped\":true}]"
        );
    }
}
