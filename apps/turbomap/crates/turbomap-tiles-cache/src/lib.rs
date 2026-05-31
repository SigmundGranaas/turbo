//! Disk-persistent `TileSource` adapter.
//!
//! Wraps any inner [`TileSource`]. On request:
//! 1. If the tile is on disk, return its bytes.
//! 2. Otherwise call the inner source, save the bytes atomically, and return
//!    them.
//!
//! The adapter never mutates inner state and is `Send + Sync` whenever the
//! inner is. Disk write errors are logged and swallowed — caching is a
//! best-effort optimisation, not a correctness gate.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use turbomap_core::{RasterFormat, RasterTile, TileError, TileId, TileSource};

pub struct DiskCachedSource<S: TileSource> {
    inner: S,
    root_dir: PathBuf,
}

impl<S: TileSource> DiskCachedSource<S> {
    /// Create a cache rooted at `root_dir`. The directory is created if it
    /// doesn't exist.
    pub fn new(inner: S, root_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let root_dir = root_dir.into();
        fs::create_dir_all(&root_dir)?;
        Ok(Self { inner, root_dir })
    }

    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// `<root>/<z>/<x>/<y>` — the file path a tile is stored at. Public so
    /// tests and tools can inspect or pre-warm the cache.
    pub fn tile_path(&self, tile: TileId) -> PathBuf {
        let mut p = self.root_dir.clone();
        p.push(tile.z.to_string());
        p.push(tile.x.to_string());
        p.push(tile.y.to_string());
        p
    }

    fn try_save(path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Atomic: write to a sibling .tmp then rename. Otherwise an
        // interrupted write would leave a corrupt half-tile on disk that
        // future runs would happily serve.
        let tmp = path.with_extension("tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

impl<S: TileSource> TileSource for DiskCachedSource<S> {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        let path = self.tile_path(tile);
        if let Ok(bytes) = fs::read(&path) {
            return Ok(RasterTile {
                bytes,
                format: self.inner.raster_format(),
            });
        }
        let raster = self.inner.request(tile)?;
        if let Err(e) = Self::try_save(&path, &raster.bytes) {
            log::warn!("disk cache write failed for {tile:?} at {path:?}: {e}");
        }
        Ok(raster)
    }

    fn min_zoom(&self) -> u8 {
        self.inner.min_zoom()
    }

    fn max_zoom(&self) -> u8 {
        self.inner.max_zoom()
    }

    fn raster_format(&self) -> RasterFormat {
        self.inner.raster_format()
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a developer composes `DiskCachedSource::new(inner,
    //! dir)` and expects (a) the inner source is hit at most once per tile,
    //! (b) the cache survives the adapter being dropped and recreated, and
    //! (c) different tile IDs don't collide on disk.

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;
    use turbomap_core::RasterFormat;

    /// A test source that records how many times it was hit and returns a
    /// deterministic byte string per tile.
    struct CountingSource {
        calls: Arc<AtomicUsize>,
    }

    impl TileSource for CountingSource {
        fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Encode the tile coords in the bytes so the test can verify the
            // right tile is served on a hit.
            let payload = format!("z{}x{}y{}", tile.z, tile.x, tile.y);
            Ok(RasterTile {
                bytes: payload.into_bytes(),
                format: RasterFormat::Png,
            })
        }
        fn raster_format(&self) -> RasterFormat {
            RasterFormat::Png
        }
    }

    fn fixture() -> (TempDir, Arc<AtomicUsize>, DiskCachedSource<CountingSource>) {
        let dir = TempDir::new().expect("tempdir");
        let calls = Arc::new(AtomicUsize::new(0));
        let cache = DiskCachedSource::new(
            CountingSource {
                calls: calls.clone(),
            },
            dir.path(),
        )
        .expect("cache builds");
        (dir, calls, cache)
    }

    #[test]
    fn first_request_is_a_miss_and_returns_inner_bytes() {
        let (_dir, calls, cache) = fixture();
        let tile = TileId::new(11, 1054, 706);
        let r = cache.request(tile).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(r.bytes, b"z11x1054y706");
        assert_eq!(r.format, RasterFormat::Png);
    }

    #[test]
    fn second_request_for_same_tile_is_a_hit_inner_is_not_called() {
        let (_dir, calls, cache) = fixture();
        let tile = TileId::new(11, 1054, 706);
        let first = cache.request(tile).unwrap();
        let second = cache.request(tile).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "inner must be called exactly once"
        );
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(second.format, RasterFormat::Png);
    }

    #[test]
    fn distinct_tiles_do_not_collide_on_disk() {
        let (_dir, calls, cache) = fixture();
        let a = TileId::new(11, 1054, 706);
        let b = TileId::new(11, 1054, 707);
        assert_eq!(cache.request(a).unwrap().bytes, b"z11x1054y706");
        assert_eq!(cache.request(b).unwrap().bytes, b"z11x1054y707");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // And both are hits the second time.
        assert_eq!(cache.request(a).unwrap().bytes, b"z11x1054y706");
        assert_eq!(cache.request(b).unwrap().bytes, b"z11x1054y707");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cache_persists_across_adapter_instances() {
        // The defining promise of a *disk* cache: a fresh process should
        // serve the same tile from disk without ever touching the inner.
        let dir = TempDir::new().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let tile = TileId::new(11, 1054, 706);
        {
            let cache = DiskCachedSource::new(
                CountingSource {
                    calls: calls.clone(),
                },
                dir.path(),
            )
            .unwrap();
            let _ = cache.request(tile).unwrap();
        } // first cache dropped

        // Second instance, same dir, fresh CountingSource counter.
        let calls2 = Arc::new(AtomicUsize::new(0));
        let cache2 = DiskCachedSource::new(
            CountingSource {
                calls: calls2.clone(),
            },
            dir.path(),
        )
        .unwrap();
        let r = cache2.request(tile).unwrap();
        assert_eq!(r.bytes, b"z11x1054y706");
        assert_eq!(
            calls2.load(Ordering::SeqCst),
            0,
            "second instance must not hit inner — file was already on disk"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn inner_error_is_propagated_and_nothing_is_saved() {
        // If the inner fails we must return the error AND leave the disk
        // unchanged, so the next request retries instead of serving a
        // half-written file.
        struct AlwaysFails;
        impl TileSource for AlwaysFails {
            fn request(&self, _tile: TileId) -> Result<RasterTile, TileError> {
                Err(TileError::Network("simulated".into()))
            }
        }
        let dir = TempDir::new().unwrap();
        let cache = DiskCachedSource::new(AlwaysFails, dir.path()).unwrap();
        let tile = TileId::new(5, 1, 1);
        assert!(cache.request(tile).is_err());
        assert!(
            !cache.tile_path(tile).exists(),
            "no file should be on disk after a failed fetch"
        );
    }
}
