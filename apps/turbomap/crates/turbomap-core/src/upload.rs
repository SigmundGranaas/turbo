//! **Uploads, recorded — and flushed through the host's uploader.**
//!
//! The renderer never writes to a `wgpu::Queue` itself. Every buffer and
//! texture write — a tile's texture at ingest, the text atlas, a frame's
//! uniforms — is recorded here in order, and [`crate::map::Map::render`]
//! flushes them through the [`Uploader`] its host passes in, before the host
//! submits the frame's encoder.
//!
//! A queue write takes effect at the next submit whenever it was issued, so
//! recording now and flushing at the end of `render` is the same frame the
//! renderer drew before. What it buys is that the host decides how a write
//! reaches the GPU: straight to its queue ([`QueueUploader`]),
//! or through an embedding application's budgeted uploader (Edits'
//! `GpuView` frames lend no queue at all).
//!
//! A refusal is an error today, never a partial frame: deferring a write
//! while the same frame draws what it wrote would put an uninitialised
//! texture on screen. Deferral waits for per-write readiness in the draw
//! path.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// The host would not take a write this frame (a byte budget, say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadRefused {
    /// Bytes of the write that was refused.
    pub bytes: u64,
    /// Writes still pending, including the refused one.
    pub pending: usize,
}

impl std::fmt::Display for UploadRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the host refused a {}-byte upload with {} still pending — the renderer does not yet defer uploads safely",
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
    fn bytes(&self) -> u64 {
        match self {
            Write::Buffer { data, .. } | Write::Texture { data, .. } => data.len() as u64,
        }
    }
}

/// The renderer's write log. Cheap to clone (shared); written from any
/// pipeline with the same calls a `wgpu::Queue` takes.
#[derive(Clone)]
pub struct UploadQueue {
    writes: Arc<Mutex<VecDeque<Write>>>,
    timestamp_period: f32,
}

impl UploadQueue {
    /// `timestamp_period`: the device queue's `get_timestamp_period()`, for
    /// GPU timing — the one thing the renderer read off a queue besides writes.
    pub fn new(timestamp_period: f32) -> Self {
        Self {
            writes: Arc::new(Mutex::new(VecDeque::new())),
            timestamp_period,
        }
    }

    fn log(&self) -> MutexGuard<'_, VecDeque<Write>> {
        self.writes.lock().unwrap_or_else(|_| {
            panic!("the renderer's upload log is poisoned: a write or flush panicked (a bug; the first panic is the cause)")
        })
    }

    pub fn write_buffer(&self, dst: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.log().push_back(Write::Buffer {
            dst: dst.clone(),
            offset,
            data: data.to_vec(),
        });
    }

    pub fn write_texture(
        &self,
        dst: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        self.log().push_back(Write::Texture {
            dst: dst.texture.clone(),
            mip_level: dst.mip_level,
            origin: dst.origin,
            aspect: dst.aspect,
            data: data.to_vec(),
            layout,
            size,
        });
    }

    pub fn get_timestamp_period(&self) -> f32 {
        self.timestamp_period
    }

    /// Writes recorded and not yet flushed.
    pub fn pending(&self) -> usize {
        self.log().len()
    }

    /// Hand every recorded write to `uploader`, in record order. On a refusal
    /// the refused write and everything after it stay recorded, and the
    /// refusal is returned.
    ///
    /// The log's lock is never held while the uploader runs: one write is
    /// taken, the lock released, the write handed over, and on a refusal put
    /// back at the front. An uploader that records into this log again
    /// (directly, or through something it calls) waits for nothing.
    pub fn flush(&self, uploader: &mut dyn Uploader) -> Result<(), UploadRefused> {
        loop {
            let Some(w) = self.log().pop_front() else {
                return Ok(());
            };
            let ok = match &w {
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
            };
            if ok.is_err() {
                let bytes = w.bytes();
                let mut log = self.log();
                log.push_front(w);
                return Err(UploadRefused {
                    bytes,
                    pending: log.len(),
                });
            }
        }
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
        log.flush(&mut up).expect("taken");
        assert_eq!(up.got, [0, 32]);
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
            .flush(&mut first)
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
        log.flush(&mut second).expect("taken now");
        assert_eq!(
            second.got,
            ["buf@16:16"],
            "resumes where it stopped, nothing lost or repeated"
        );
        assert_eq!(log.pending(), 0);
    }
}
