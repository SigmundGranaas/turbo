//! Optional GPU-side frame timing via `Features::TIMESTAMP_QUERY`.
//!
//! Lifecycle, per frame:
//!   1. `try_drain` is called at the top of `Map::render`. If the
//!      previous frame's readback completed, decode its two u64
//!      timestamps and store the difference as `last_duration_ns`.
//!   2. `begin(encoder)` writes the start timestamp.
//!   3. Render passes run.
//!   4. `end(encoder)` writes the end timestamp, resolves the
//!      `QuerySet` into a GPU-only buffer, then copies into the
//!      `MAP_READ` readback buffer.
//!   5. After the host submits the encoder, `kick_async` arms the
//!      readback's `map_async` so the next frame's `try_drain`
//!      finds the result ready.
//!
//! Because the queue.submit is host-owned (the renderer hands the
//! host a built encoder), this struct does NOT call `submit` itself.
//! It also does NOT call `device.poll()` — that's the host's job
//! (winit's redraw flow does it implicitly via `surface.present`,
//! but the offscreen snapshot has to call it explicitly).
//!
//! If `TIMESTAMP_QUERY` isn't available on the device, `new` returns
//! `None` and the renderer silently skips timestamping. The CPU
//! frame time in `FrameMetrics` is always populated regardless.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(crate) struct GpuTimestamps {
    query_set: wgpu::QuerySet,
    /// Plain-GPU buffer the query writes resolve into (QUERY_RESOLVE).
    /// Distinct from the readback buffer because QUERY_RESOLVE and
    /// MAP_READ are not co-usable on the same buffer.
    resolve_buffer: wgpu::Buffer,
    /// MAP_READ buffer the resolved timestamps get copied into so the
    /// CPU can read them next frame.
    readback_buffer: wgpu::Buffer,
    /// Nanoseconds per GPU timestamp tick — comes from
    /// `Queue::get_timestamp_period`.
    period_ns: f32,
    /// Set true after `kick_async`; cleared once the callback fires.
    mapping_in_flight: Arc<AtomicBool>,
    /// Set true when the readback buffer holds resolved bytes the CPU
    /// can read. Cleared after `try_drain` consumes the value.
    ready: Arc<AtomicBool>,
    /// Latest measured frame GPU time. Reset only when a fresh
    /// reading arrives; lingers between frames so the metric is
    /// stable to display.
    pub last_duration_ns: u64,
    /// True if `end` has been called this frame; gates `kick_async`.
    has_pending_resolve: bool,
}

impl GpuTimestamps {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        // wgpu 22 split TIMESTAMP_QUERY into multiple slots:
        // - TIMESTAMP_QUERY: lets us allocate `QuerySet`s of type
        //   `Timestamp` and resolve them.
        // - TIMESTAMP_QUERY_INSIDE_ENCODERS: lets us call
        //   `encoder.write_timestamp` between passes.
        // We need both. (TIMESTAMP_QUERY_INSIDE_PASSES is a third
        // gate for inside a render pass; we write at pass boundaries
        // only, so we don't need it.)
        let needed = wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !device.features().contains(needed) {
            return None;
        }
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("turbomap-frame-timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-frame-timestamps-resolve"),
            size: 16, // 2 × u64
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-frame-timestamps-readback"),
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            period_ns: queue.get_timestamp_period(),
            mapping_in_flight: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            last_duration_ns: 0,
            has_pending_resolve: false,
        })
    }

    /// If the previous frame's `map_async` completed, decode the
    /// timestamps and stash the new measurement. Non-blocking.
    pub fn try_drain(&mut self) {
        if !self.ready.load(Ordering::Acquire) {
            return;
        }
        let slice = self.readback_buffer.slice(..);
        let data = slice.get_mapped_range();
        // Two little-endian u64 timestamps in raw GPU ticks.
        let mut t0 = [0u8; 8];
        let mut t1 = [0u8; 8];
        t0.copy_from_slice(&data[0..8]);
        t1.copy_from_slice(&data[8..16]);
        let t0 = u64::from_le_bytes(t0);
        let t1 = u64::from_le_bytes(t1);
        drop(data);
        self.readback_buffer.unmap();
        // Some backends report monotonically-decreasing or wrapping
        // ticks if the queue resets; treat that as a noisy frame.
        let ticks = t1.saturating_sub(t0);
        self.last_duration_ns = (ticks as f64 * self.period_ns as f64) as u64;
        self.ready.store(false, Ordering::Release);
        self.mapping_in_flight.store(false, Ordering::Release);
    }

    /// Returns true if the readback buffer is currently free (not
    /// pending an async map). The encoder-side resolve+copy is only
    /// safe to issue when the buffer is unmapped — otherwise wgpu
    /// raises "Buffer is still mapped" on submit. Gated work is
    /// silently skipped this frame; we just don't get a sample.
    fn buffer_free(&self) -> bool {
        !self.mapping_in_flight.load(Ordering::Acquire)
    }

    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.buffer_free() {
            return;
        }
        encoder.write_timestamp(&self.query_set, 0);
        self.has_pending_resolve = true;
    }

    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.has_pending_resolve {
            return;
        }
        encoder.write_timestamp(&self.query_set, 1);
        encoder.resolve_query_set(&self.query_set, 0..2, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(&self.resolve_buffer, 0, &self.readback_buffer, 0, 16);
    }

    /// After the host submits the encoder built this frame, arm a
    /// non-blocking readback. The callback flips `ready` when the
    /// GPU finishes the copy and the host calls `device.poll`.
    pub fn kick_async(&mut self) {
        if !self.has_pending_resolve {
            return;
        }
        self.has_pending_resolve = false;
        // Take ownership of the buffer; subsequent begin() will
        // short-circuit until try_drain releases it.
        self.mapping_in_flight.store(true, Ordering::Release);
        let ready = self.ready.clone();
        let slice = self.readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |res| {
            if res.is_ok() {
                ready.store(true, Ordering::Release);
            }
            // On failure the buffer stays "in flight" and we never
            // sample again until reset — acceptable for a debug
            // metric.
        });
    }
}
