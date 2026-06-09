//! Performance summaries over a recorded session.
//!
//! Numbers from the software rasteriser are *relative*: they catch
//! regressions (a frame suddenly costing 3× after a change) and expose
//! how cost scales with scene complexity, but they are not mobile-GPU
//! milliseconds. The same instrumentation runs unchanged on a real
//! adapter when one is present.

use serde::Serialize;

use crate::session::FrameStats;

#[derive(Debug, Clone, Serialize)]
pub struct PerfSummary {
    pub frames: usize,
    pub cpu_ms_p50: f64,
    pub cpu_ms_p95: f64,
    pub cpu_ms_max: f64,
    /// Worst blank-coverage seen across the session (loading quality).
    pub worst_blank_frac: f64,
    /// Total tiles delivered over the session.
    pub tiles_delivered: u64,
}

impl PerfSummary {
    pub fn from_stats(stats: &[FrameStats]) -> Self {
        let mut cpu: Vec<f64> = stats.iter().map(|s| s.cpu_ms).collect();
        cpu.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
        let pick = |q: f64| -> f64 {
            if cpu.is_empty() {
                return 0.0;
            }
            let idx = ((cpu.len() as f64 - 1.0) * q).round() as usize;
            cpu[idx]
        };
        Self {
            frames: stats.len(),
            cpu_ms_p50: pick(0.50),
            cpu_ms_p95: pick(0.95),
            cpu_ms_max: cpu.last().copied().unwrap_or(0.0),
            worst_blank_frac: stats
                .iter()
                .map(|s| s.blank_frac)
                .fold(0.0, f64::max),
            tiles_delivered: stats.iter().map(|s| s.delivered as u64).sum(),
        }
    }
}
