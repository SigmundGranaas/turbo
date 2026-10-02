//! **Uploads, recorded — and flushed through the host's uploader.**
//!
//! The renderer never writes to a `wgpu::Queue` itself. Every buffer and
//! texture write is recorded here, and [`crate::map::Map::render`] hands them
//! to the [`Uploader`] its host passes in, before the host submits the
//! frame's encoder. A queue write takes effect at the next submit whenever it
//! was issued, so this is the same frame the renderer drew before; what it
//! buys is that the host decides how a write reaches the GPU — straight to
//! its queue ([`QueueUploader`]), or through an embedding application's
//! budgeted uploader (Edits' `GpuView` frames lend no queue at all).
//!
//! # Two logs, because a host's budget can run out
//!
//! - **Essential** writes are everything else — construction-time
//!   placeholders, atlases, a frame's uniforms. The frame is drawn with them,
//!   so they are handed over first, after recording, and must be taken: a
//!   refusal is [`UploadRefused`], an error, never a frame drawn with stale
//!   uniforms.
//! - **Deferrable** writes are tile data at ingest
//!   ([`UploadQueue::write_texture_deferrable`]). Each returns an
//!   [`UploadTicket`]; the tile cache treats a tile whose ticket is not
//!   [`UploadQueue::is_ready`] as not resident, so the draw falls back to its
//!   ancestor exactly as for a tile still in flight. They are handed over
//!   after the essentials, in whatever the host's [`Uploader::remaining`]
//!   leaves; what does not fit waits for the next frame. A tile handed over
//!   this frame is drawn from the next.
//!
//! [`UploadQueue::flush`] is that order, and the only way `render` hands
//! anything over. It used to be the reverse — tiles first, keeping back what
//! the *previous* frame's essentials had needed — so a frame whose uniforms
//! outgrew the last one's had them refused while a pan brought tiles in:
//! the map failed its own frame (measured through Edits' 1 MiB-a-frame
//! uploader: 16 of 20 pans of 540 frames). A frame's essential bytes are
//! known exactly only after recording, which is when they now go first.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// The host would not take an essential write this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadRefused {
    /// Bytes of the write that was refused.
    pub bytes: u64,
    /// Essential writes still pending, including the refused one.
    pub pending: usize,
}

impl std::fmt::Display for UploadRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the host refused an essential {}-byte upload with {} still pending — a frame's uniforms and atlases \
             must fit the host's per-frame upload budget",
            self.bytes, self.pending
        )
    }
}

impl std::error::Error for UploadRefused {}

/// An uploader's answer when it will not take a write now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused;

/// How recorded writes reach the GPU. The host's to implement.
pub trait Uploader {
    /// Bytes this uploader will still take this frame; `None` for no limit.
    fn remaining(&self) -> Option<u64> {
        None
    }
    fn write_buffer(&mut self, dst: &wgpu::Buffer, offset: u64, data: &[u8])
        -> Result<(), Refused>;
    fn write_texture(
        &mut self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) -> Result<(), Refused>;
}

/// A queue takes every write: the standalone hosts' uploader. A wrapper
/// rather than an impl on `wgpu::Queue`, because hosts share their queue
/// (`Arc<wgpu::Queue>`) and an uploader is borrowed mutably.
pub struct QueueUploader<'a>(pub &'a wgpu::Queue);

impl Uploader for QueueUploader<'_> {
    fn write_buffer(
        &mut self,
        dst: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<(), Refused> {
        self.0.write_buffer(dst, offset, data);
        Ok(())
    }
    fn write_texture(
        &mut self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) -> Result<(), Refused> {
        self.0.write_texture(dst, data, layout, size);
        Ok(())
    }
}

/// The most one deferrable write hands over at once: tile uploads are split
/// into row bands of at most this (see [`UploadQueue::write_texture_deferrable`]).
pub const DEFERRABLE_BAND_BYTES: u64 = 256 * 1024;

/// `(first_row, rows, byte range)` bands of a 2D write, each at most
/// [`DEFERRABLE_BAND_BYTES`] (at least one row). A write with no row stride
/// or more than one layer stays whole.
fn bands(
    layout: &wgpu::TexelCopyBufferLayout,
    size: wgpu::Extent3d,
    len: usize,
) -> Vec<(u32, u32, std::ops::Range<usize>)> {
    let whole = vec![(0, size.height, layout.offset as usize..len)];
    let Some(stride) = layout.bytes_per_row else {
        return whole;
    };
    if size.depth_or_array_layers != 1 || size.height <= 1 {
        return whole;
    }
    let rows_per_band = ((DEFERRABLE_BAND_BYTES / u64::from(stride)).max(1)) as u32;
    let mut out = Vec::new();
    let mut row = 0;
    while row < size.height {
        let rows = rows_per_band.min(size.height - row);
        let start = layout.offset as usize + row as usize * stride as usize;
        let end = (start + rows as usize * stride as usize).min(len);
        out.push((row, rows, start..end));
        row += rows;
    }
    out
}

/// Proof that a deferrable write was recorded; ready once handed to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UploadTicket(u64);

enum Write {
    Buffer {
        dst: wgpu::Buffer,
        offset: u64,
        data: Vec<u8>,
    },
    Texture {
        dst: wgpu::Texture,
        mip_level: u32,
        origin: wgpu::Origin3d,
        aspect: wgpu::TextureAspect,
        data: Vec<u8>,
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    },
}

impl Write {
    /// Whether `self`, recorded later, overwrites exactly what `earlier`
    /// wrote — same resource, same region — so `earlier` need never reach
    /// the GPU: only the last write's bytes are there at submit.
    fn supersedes(&self, earlier: &Write) -> bool {
        match (self, earlier) {
            (
                Write::Buffer {
                    dst: a,
                    offset: oa,
                    data: da,
                },
                Write::Buffer {
                    dst: b,
                    offset: ob,
                    data: db,
                },
            ) => a == b && oa == ob && da.len() == db.len(),
            (
                Write::Texture {
                    dst: a,
                    mip_level: ma,
                    origin: oa,
                    aspect: aa,
                    size: sa,
                    ..
                },
                Write::Texture {
                    dst: b,
                    mip_level: mb,
                    origin: ob,
                    aspect: ab,
                    size: sb,
                    ..
                },
            ) => a == b && ma == mb && oa == ob && aa == ab && sa == sb,
            _ => false,
        }
    }

    fn bytes(&self) -> u64 {
        match self {
            Write::Buffer { data, .. } | Write::Texture { data, .. } => data.len() as u64,
        }
    }
}

struct Logs {
    essential: VecDeque<Write>,
    deferrable: VecDeque<(u64, Write)>,
    next_ticket: u64,
    /// Every deferrable write with a ticket below this has been handed over.
    ready_below: u64,
    /// `ready_below` as the last recording saw it: every write below this has
    /// been drawn. A write is handed over after the frame that recorded
    /// ([`UploadQueue::flush`]), so it is first drawn by the next one.
    drawn_below: u64,
}

/// The renderer's write log. Cheap to clone (shared); written from any
/// pipeline with the same calls a `wgpu::Queue` takes.
#[derive(Clone)]
pub struct UploadQueue {
    logs: Arc<Mutex<Logs>>,
    timestamp_period: f32,
}

impl UploadQueue {
    /// `timestamp_period`: the device queue's `get_timestamp_period()`, for
    /// GPU timing — the one thing the renderer read off a queue besides writes.
    pub fn new(timestamp_period: f32) -> Self {
        let logs = Logs {
            essential: VecDeque::new(),
            deferrable: VecDeque::new(),
            next_ticket: 0,
            ready_below: 0,
            drawn_below: 0,
        };
        Self {
            logs: Arc::new(Mutex::new(logs)),
            timestamp_period,
        }
    }

    fn log(&self) -> MutexGuard<'_, Logs> {
        self.logs.lock().unwrap_or_else(|_| {
            panic!("the renderer's upload log is poisoned: a write or flush panicked (a bug; the first panic is the cause)")
        })
    }

    /// An essential buffer write (see the module doc).
    pub fn write_buffer(&self, dst: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.record_essential(Write::Buffer {
            dst: dst.clone(),
            offset,
            data: data.to_vec(),
        });
    }

    /// Record an essential write, dropping any still-queued one it fully
    /// overwrites (same resource, same region): only the final bytes are on
    /// the GPU at submit, so the frame is unchanged — and a host's budget is
    /// not spent on, e.g., three copies of a height field rewritten per DEM
    /// tile ingested (measured: 256 KiB each).
    fn record_essential(&self, w: Write) {
        let mut logs = self.log();
        logs.essential.retain(|earlier| !w.supersedes(earlier));
        logs.essential.push_back(w);
    }

    /// An essential texture write (see the module doc).
    pub fn write_texture(
        &self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        self.record_essential(texture_write(dst, data, layout, size));
    }

    /// A tile's data, which may wait for a later frame: the returned ticket
    /// is [`Self::is_ready`] once the write has been handed to the host, and
    /// what it wrote must not be drawn before then.
    ///
    /// Split into row bands of at most [`DEFERRABLE_BAND_BYTES`]: the flush
    /// stops at the first write that does not fit, so a write larger than a
    /// host's budget (less the reserve) would wait forever — and the engine,
    /// wanting frames while uploads wait, would spin on it. A 512² RGBA base
    /// level is 1 MiB; in bands, any tile fits any budget above one band plus
    /// the reserve. The returned ticket is the last band's (bands are handed
    /// over in order, so it covers them all).
    pub fn write_texture_deferrable(
        &self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) -> UploadTicket {
        let mut logs = self.log();
        let bands = bands(&layout, size, data.len());
        let mut last = None;
        for (first_row, rows, range) in bands {
            let t = logs.next_ticket;
            logs.next_ticket += 1;
            let band_dst = wgpu::TexelCopyTextureInfo {
                texture: dst.texture,
                mip_level: dst.mip_level,
                origin: wgpu::Origin3d {
                    y: dst.origin.y + first_row,
                    ..dst.origin
                },
                aspect: dst.aspect,
            };
            let band_layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: layout.bytes_per_row,
                rows_per_image: Some(rows),
            };
            let band_size = wgpu::Extent3d {
                height: rows,
                ..size
            };
            logs.deferrable.push_back((
                t,
                texture_write(band_dst, &data[range], band_layout, band_size),
            ));
            last = Some(t);
        }
        UploadTicket(last.expect("a texture write has at least one band"))
    }

    /// Whether the write behind `ticket` has been handed to the host.
    pub fn is_ready(&self, ticket: UploadTicket) -> bool {
        ticket.0 < self.log().ready_below
    }

    pub fn get_timestamp_period(&self) -> f32 {
        self.timestamp_period
    }

    /// Writes recorded and not yet handed over (both logs).
    pub fn pending(&self) -> usize {
        let logs = self.log();
        logs.essential.len() + logs.deferrable.len()
    }

    /// Deferrable writes still waiting for room in an uploader.
    pub fn pending_deferrable(&self) -> usize {
        self.log().deferrable.len()
    }

    /// **Deferrable writes no frame has drawn yet** — still waiting for room,
    /// or handed over since the last recording. A frame is wanted while any
    /// are: a write handed over after a frame recorded is first drawn by the
    /// next frame, and a host that stops at zero *waiting* writes would never
    /// draw the last ones handed over (Edits' map_tiles, 2026-10-03: eight
    /// stories whose tiles arrived and were never shown).
    pub fn undrawn_deferrable(&self) -> usize {
        let logs = self.log();
        logs.deferrable.len() + (logs.ready_below - logs.drawn_below) as usize
    }

    /// A frame is recording: every write handed over so far is drawn by it.
    /// Called by the renderer at the start of each recording.
    pub fn recording(&self) {
        let mut logs = self.log();
        logs.drawn_below = logs.ready_below;
    }

    /// Hand over deferrable writes, in order, while `uploader` has room
    /// beyond the reserve the essential writes will need. Stops at the first
    /// that does not fit, leaving it and the rest for a later frame. The lock
    /// is never held while the uploader runs.
    pub fn flush_deferrable(&self, uploader: &mut dyn Uploader) {
        loop {
            let (ticket, w) = {
                let mut logs = self.log();
                // Room for every essential write still recorded: none, when
                // called from `flush`, which hands those over first. Kept so
                // a caller that flushes tiles alone cannot starve them.
                let reserve = logs.essential.iter().map(Write::bytes).sum::<u64>();
                let Some((_, front)) = logs.deferrable.front() else {
                    return;
                };
                if let Some(left) = uploader.remaining() {
                    if front.bytes().saturating_add(reserve) > left {
                        return;
                    }
                }
                logs.deferrable.pop_front().expect("front checked")
            };
            if hand_over(&w, uploader).is_err() {
                // It fitted by `remaining()` and was refused anyway: keep it
                // for the next frame.
                self.log().deferrable.push_front((ticket, w));
                return;
            }
            self.log().ready_below = ticket + 1;
        }
    }

    /// **Hand this frame's writes to `uploader`**: every essential write (they
    /// must be taken — see [`Self::flush_essential`]), then tile data in what
    /// the uploader has left ([`Self::flush_deferrable`]). Called once a frame,
    /// after recording.
    pub fn flush(&self, uploader: &mut dyn Uploader) -> Result<(), UploadRefused> {
        self.flush_essential(uploader)?;
        self.flush_deferrable(uploader);
        Ok(())
    }

    /// Hand over every essential write, in record order. They must all be
    /// taken; on a refusal the refused write and those after it stay
    /// recorded and the refusal is returned. The lock is never held while
    /// the uploader runs, so an uploader that records into this log again
    /// waits for nothing.
    pub fn flush_essential(&self, uploader: &mut dyn Uploader) -> Result<(), UploadRefused> {
        loop {
            let Some(w) = self.log().essential.pop_front() else {
                return Ok(());
            };
            if hand_over(&w, uploader).is_err() {
                let bytes = w.bytes();
                let mut logs = self.log();
                logs.essential.push_front(w);
                return Err(UploadRefused {
                    bytes,
                    pending: logs.essential.len(),
                });
            }
        }
    }
}

fn texture_write(
    dst: wgpu::TexelCopyTextureInfo<'_>,
    data: &[u8],
    layout: wgpu::TexelCopyBufferLayout,
    size: wgpu::Extent3d,
) -> Write {
    Write::Texture {
        dst: dst.texture.clone(),
        mip_level: dst.mip_level,
        origin: dst.origin,
        aspect: dst.aspect,
        data: data.to_vec(),
        layout,
        size,
    }
}

fn hand_over(w: &Write, uploader: &mut dyn Uploader) -> Result<(), Refused> {
    match w {
        Write::Buffer { dst, offset, data } => uploader.write_buffer(dst, *offset, data),
        Write::Texture {
            dst,
            mip_level,
            origin,
            aspect,
            data,
            layout,
            size,
        } => uploader.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: dst,
                mip_level: *mip_level,
                origin: *origin,
                aspect: *aspect,
            },
            data,
            *layout,
            *size,
        ),
    }
}

/// The clouds pass records into the same log as every other pass.
impl turbomap_clouds::QueueWrites for UploadQueue {
    fn write_buffer(&self, dst: &wgpu::Buffer, offset: u64, data: &[u8]) {
        UploadQueue::write_buffer(self, dst, offset, data);
    }
    fn write_texture(
        &self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        UploadQueue::write_texture(self, dst, data, layout, size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records what reached it, and refuses from the `refuse_from`th write on.
    struct Recording {
        got: Vec<String>,
        refuse_from: usize,
    }

    impl Uploader for Recording {
        fn write_buffer(
            &mut self,
            _dst: &wgpu::Buffer,
            offset: u64,
            data: &[u8],
        ) -> Result<(), Refused> {
            if self.got.len() >= self.refuse_from {
                return Err(Refused);
            }
            self.got.push(format!("buf@{offset}:{}", data.len()));
            Ok(())
        }
        fn write_texture(
            &mut self,
            _dst: wgpu::TexelCopyTextureInfo<'_>,
            data: &[u8],
            _layout: wgpu::TexelCopyBufferLayout,
            _size: wgpu::Extent3d,
        ) -> Result<(), Refused> {
            if self.got.len() >= self.refuse_from {
                return Err(Refused);
            }
            self.got.push(format!("tex:{}", data.len()));
            Ok(())
        }
    }

    fn device() -> Option<wgpu::Device> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            ..Default::default()
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .ok()
            .map(|(d, _)| d)
    }

    /// An uploader that writes into the log it is flushing (as a host that
    /// re-records a refused tile might): no deadlock, and the new write is
    /// flushed after the ones before it.
    #[test]
    fn an_uploader_may_record_into_the_log_it_is_flushing() {
        let Some(device) = device() else { return };
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let log = UploadQueue::new(1.0);
        log.write_buffer(&buf, 0, &[0; 4]);
        struct Reentrant<'a> {
            log: &'a UploadQueue,
            buf: &'a wgpu::Buffer,
            got: Vec<u64>,
        }
        impl Uploader for Reentrant<'_> {
            fn write_buffer(
                &mut self,
                _: &wgpu::Buffer,
                offset: u64,
                _: &[u8],
            ) -> Result<(), Refused> {
                if self.got.is_empty() {
                    self.log.write_buffer(self.buf, 32, &[0; 4]);
                }
                self.got.push(offset);
                Ok(())
            }
            fn write_texture(
                &mut self,
                _: wgpu::TexelCopyTextureInfo<'_>,
                _: &[u8],
                _: wgpu::TexelCopyBufferLayout,
                _: wgpu::Extent3d,
            ) -> Result<(), Refused> {
                Ok(())
            }
        }
        let mut up = Reentrant {
            log: &log,
            buf: &buf,
            got: vec![],
        };
        log.flush_essential(&mut up).expect("taken");
        assert_eq!(up.got, [0, 32]);
    }

    /// A write that fully overwrites a queued one replaces it; a partial
    /// overlap does not (both must reach the GPU, in order).
    #[test]
    fn a_full_overwrite_supersedes_the_queued_write_and_a_partial_one_does_not() {
        let Some(device) = device() else { return };
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let log = UploadQueue::new(1.0);
        log.write_buffer(&buf, 0, &[1; 16]);
        log.write_buffer(&buf, 0, &[2; 8]); // partial: both stay
        log.write_buffer(&buf, 0, &[3; 16]); // full overwrite of the first
        let mut up = Recording {
            got: vec![],
            refuse_from: usize::MAX,
        };
        log.flush_essential(&mut up).expect("taken");
        assert_eq!(
            up.got,
            ["buf@0:8", "buf@0:16"],
            "the first 16-byte write was superseded; order kept"
        );
    }

    /// A deferrable write larger than a frame's budget is not stuck: in row
    /// bands it is handed over across frames, and ready after the last.
    #[test]
    fn a_tile_larger_than_the_budget_is_handed_over_in_bands_across_frames() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            }))
        else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let log = UploadQueue::new(1.0);
        let ticket = log.write_texture_deferrable(
            texture.as_image_copy(),
            &vec![7u8; 512 * 512 * 4], // 1 MiB: more than a frame's 512 KiB
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512 * 4),
                rows_per_image: Some(512),
            },
            wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
        );
        struct Budget<'a>(&'a wgpu::Queue, u64);
        impl Uploader for Budget<'_> {
            fn remaining(&self) -> Option<u64> {
                Some(self.1)
            }
            fn write_buffer(&mut self, _: &wgpu::Buffer, _: u64, _: &[u8]) -> Result<(), Refused> {
                Err(Refused)
            }
            fn write_texture(
                &mut self,
                dst: wgpu::TexelCopyTextureInfo<'_>,
                data: &[u8],
                layout: wgpu::TexelCopyBufferLayout,
                size: wgpu::Extent3d,
            ) -> Result<(), Refused> {
                self.1 = self.1.checked_sub(data.len() as u64).ok_or(Refused)?;
                self.0.write_texture(dst, data, layout, size);
                Ok(())
            }
        }
        let mut frames = 0;
        while !log.is_ready(ticket) {
            frames += 1;
            assert!(
                frames <= 16,
                "a 1 MiB tile never finished at 512 KiB a frame: it is stuck"
            );
            log.flush_deferrable(&mut Budget(&queue, 512 * 1024));
        }
        assert_eq!(frames, 2, "four 256 KiB bands at two a frame");
        assert_eq!(log.pending_deferrable(), 0);
        queue.submit([]);
    }

    /// **A frame's own writes are never starved by tiles.** Tile data that
    /// would fill the host's whole budget is waiting, and this frame records
    /// more essential bytes than the last one did — what a pan does when
    /// tiles arrive and the frame's uniforms grow. Every essential write is
    /// taken; the tiles get what is left and the rest waits. Until `flush`
    /// handed essentials over first, the reserve was the previous frame's
    /// essential bytes and this frame's grown uniforms were refused.
    #[test]
    fn a_frames_essentials_are_taken_before_tiles_even_when_they_grew() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            }))
        else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4096,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 256, height: 256, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        struct Budget<'a>(&'a wgpu::Queue, u64);
        impl Uploader for Budget<'_> {
            fn remaining(&self) -> Option<u64> {
                Some(self.1)
            }
            fn write_buffer(&mut self, dst: &wgpu::Buffer, offset: u64, data: &[u8]) -> Result<(), Refused> {
                self.1 = self.1.checked_sub(data.len() as u64).ok_or(Refused)?;
                self.0.write_buffer(dst, offset, data);
                Ok(())
            }
            fn write_texture(
                &mut self,
                dst: wgpu::TexelCopyTextureInfo<'_>,
                data: &[u8],
                layout: wgpu::TexelCopyBufferLayout,
                size: wgpu::Extent3d,
            ) -> Result<(), Refused> {
                self.1 = self.1.checked_sub(data.len() as u64).ok_or(Refused)?;
                self.0.write_texture(dst, data, layout, size);
                Ok(())
            }
        }
        let log = UploadQueue::new(1.0);
        // Frame 1: a small essential write, nothing else.
        log.write_buffer(&buf, 0, &[0; 64]);
        log.flush(&mut Budget(&queue, 256 * 1024)).expect("frame 1's essentials");
        // Frame 2: a whole budget's worth of tiles arrives, and the frame's
        // own writes grow to sixteen times frame 1's.
        let tile = log.write_texture_deferrable(
            texture.as_image_copy(),
            &vec![7u8; 256 * 256 * 4],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(256) },
            wgpu::Extent3d { width: 256, height: 256, depth_or_array_layers: 1 },
        );
        log.write_buffer(&buf, 0, &[0; 1024]);
        let mut frame = Budget(&queue, 256 * 1024);
        log.flush(&mut frame).expect("a frame's own writes are taken before any tile");
        assert_eq!(log.pending() - log.pending_deferrable(), 0, "every essential write was handed over");
        assert!(!log.is_ready(tile), "the tile did not fit what the essentials left; it waits");
        // Frame 3: the tile has the budget to itself.
        log.flush(&mut Budget(&queue, 256 * 1024)).expect("frame 3");
        assert!(log.is_ready(tile), "the tile goes the next frame");
        // Handed over after frame 3 recorded, so no frame has drawn it: a
        // frame is still wanted, or a host that renders on demand stops here
        // and the tile never appears.
        assert_eq!(log.undrawn_deferrable(), 1, "the tile was handed over and no frame has drawn it yet");
        log.recording();
        assert_eq!(log.undrawn_deferrable(), 0, "frame 4 recorded with the tile ready: it is drawn");
        queue.submit([]);
    }

    #[test]
    fn writes_reach_the_uploader_in_record_order_and_a_refusal_keeps_the_rest() {
        let Some(device) = device() else {
            eprintln!("no software adapter: the upload log is exercised by the goldens instead");
            return;
        };
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let log = UploadQueue::new(1.0);
        log.write_buffer(&buf, 0, &[0; 4]);
        log.write_buffer(&buf, 8, &[0; 8]);
        log.write_buffer(&buf, 16, &[0; 16]);

        let mut first = Recording {
            got: vec![],
            refuse_from: 2,
        };
        let refused = log
            .flush_essential(&mut first)
            .expect_err("the third write is refused");
        assert_eq!(first.got, ["buf@0:4", "buf@8:8"]);
        assert_eq!(
            refused,
            UploadRefused {
                bytes: 16,
                pending: 1
            }
        );
        assert_eq!(log.pending(), 1, "the refused write stays recorded");

        let mut second = Recording {
            got: vec![],
            refuse_from: usize::MAX,
        };
        log.flush_essential(&mut second).expect("taken now");
        assert_eq!(
            second.got,
            ["buf@16:16"],
            "resumes where it stopped, nothing lost or repeated"
        );
        assert_eq!(log.pending(), 0);
    }
}
