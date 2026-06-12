//! HTTP `TileSource` adapter.
//!
//! The host configures a URL template that uses `{z}`, `{x}`, `{y}` (or the
//! WMTS-style `{TileMatrix}`, `{TileCol}`, `{TileRow}`) placeholders, and the
//! adapter expands them at request time.

use std::time::Duration;

use turbomap_core::{
    RasterFormat, RasterTile, TileError, TileId, TileSource, VectorTile, VectorTileSource,
};

pub mod cache;
pub use cache::DiskCache;
pub mod retry;
pub use retry::RetryPolicy;

/// A fetch failure classified by whether retrying could help. `Transient`
/// covers transport errors (dropped connection, timeout) and the server-side
/// / rate-limit statuses (5xx, 408, 429); `Permanent` covers the rest (a
/// 404 for a tile that doesn't exist, a 403 auth failure) — retrying those
/// just wastes requests and backoff.
#[derive(Debug)]
enum FetchError {
    Transient(String),
    Permanent(String),
}

impl FetchError {
    fn is_transient(&self) -> bool {
        matches!(self, FetchError::Transient(_))
    }

    /// Collapse to the public error after retries are exhausted. Both map to
    /// `Network` (the host sees "this tile didn't load"); the distinction
    /// only mattered for the retry decision.
    fn into_tile_error(self) -> TileError {
        match self {
            FetchError::Transient(m) | FetchError::Permanent(m) => TileError::Network(m),
        }
    }
}

/// One HTTP GET → validated body bytes, classified for the retry loop. A
/// transport error is always transient; a non-2xx status is transient only
/// for 5xx / 408 / 429.
fn fetch_bytes(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>, FetchError> {
    let resp = client
        .get(url)
        .send()
        // A send() failure is a transport problem (DNS, connect, timeout) —
        // always worth a retry.
        .map_err(|e| FetchError::Transient(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        let msg = format!("HTTP {status} for {url}");
        return Err(if status_is_transient(status.as_u16()) {
            FetchError::Transient(msg)
        } else {
            FetchError::Permanent(msg)
        });
    }
    resp.bytes()
        .map(|b| b.to_vec())
        // A body-read failure mid-stream is a transport hiccup → transient.
        .map_err(|e| FetchError::Transient(e.to_string()))
}

/// Is a non-2xx HTTP status worth retrying? Server errors (5xx), request
/// timeout (408) and rate-limit (429) are transient; every other 4xx (a
/// missing tile, bad auth) is permanent — retrying just wastes requests.
fn status_is_transient(code: u16) -> bool {
    code >= 500 || code == 408 || code == 429
}

/// A configured HTTP raster tile source.
#[derive(Debug, Clone)]
pub struct HttpRasterSource {
    client: reqwest::blocking::Client,
    url_template: String,
    min_zoom: u8,
    max_zoom: u8,
    format: RasterFormat,
    attribution: Option<String>,
    /// For DEM sources (Mapbox Terrain-RGB), how many pixels of
    /// halo each tile carries beyond the canonical 256×256. The
    /// hillshade pipeline reads this via [`TileSource::dem_halo_px`]
    /// to map the displayed quad to the texture interior, killing
    /// edge seams.
    dem_halo_px: u32,
    retry: RetryPolicy,
}

impl HttpRasterSource {
    /// Build a source from a URL template. Placeholders supported:
    /// `{z}` / `{TileMatrix}`, `{x}` / `{TileCol}`, `{y}` / `{TileRow}`.
    pub fn new(
        url_template: impl Into<String>,
        user_agent: impl Into<String>,
        min_zoom: u8,
        max_zoom: u8,
        format: RasterFormat,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(user_agent.into())
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client,
            url_template: url_template.into(),
            min_zoom,
            max_zoom,
            format,
            attribution: None,
            dem_halo_px: 0,
            retry: RetryPolicy::default(),
        })
    }

    pub fn with_attribution(mut self, attribution: impl Into<String>) -> Self {
        self.attribution = Some(attribution.into());
        self
    }

    /// Override the transient-failure retry policy (default
    /// [`RetryPolicy::default`]; pass [`RetryPolicy::none`] to disable).
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    pub fn attribution(&self) -> Option<&str> {
        self.attribution.as_deref()
    }

    /// Declare the per-tile halo this source serves. The pipeline reads
    /// this on `add_hillshade_layer`. Setter rather than constructor
    /// argument so it composes with the existing presets.
    pub fn with_dem_halo(mut self, halo_px: u32) -> Self {
        self.dem_halo_px = halo_px;
        self
    }

    /// Norwegian Kartverket Turkart, preconfigured. The template comes from
    /// `apps/flutter/lib/features/tile_providers/data/providers/norges_kart_topo.dart`.
    pub fn kartverket_topo() -> Result<Self, reqwest::Error> {
        let user_agent = format!("turbomap/{}", env!("CARGO_PKG_VERSION"));
        Self::new(
            "https://cache.atgcp1-prod.kartverket.cloud/v1/service\
             ?layer=topo&style=default&tilematrixset=webmercator\
             &Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png\
             &TileMatrix={z}&TileCol={x}&TileRow={y}",
            user_agent,
            4,
            20,
            RasterFormat::Png,
        )
        .map(|s| s.with_attribution("© Kartverket"))
    }

    /// Our own tileserver's Mapbox-Terrain-RGB DEM endpoint. The handler
    /// lives at `apps/tileserver/crates/turbo-tiles-api/src/v1/dem.rs`
    /// and serves PNG tiles where RGB encodes elevation in metres. Pair
    /// with `Map::add_hillshade_layer(..., HillshadeStyle::default())`
    /// (which defaults to `DemEncoding::MapboxRgb`).
    ///
    /// `base_url` is the tileserver root, e.g. `https://api.turbo.app` or
    /// `http://localhost:8080` for local dev. Trailing slashes are
    /// trimmed automatically.
    pub fn turbo_terrain_rgb(base_url: &str) -> Result<Self, reqwest::Error> {
        let trimmed = base_url.trim_end_matches('/');
        // `?halo=1` asks the server for a 258×258 PNG with one pixel
        // of overscan on every side, so the GPU hillshade kernel can
        // step into the neighbour's terrain without ClampToEdge
        // seams at tile boundaries.
        let url = format!("{trimmed}/v1/dem/rgb/{{z}}/{{x}}/{{y}}.png?halo=1");
        let user_agent = format!("turbomap/{}", env!("CARGO_PKG_VERSION"));
        // DTM10 covers Norway zoom ~6 onward usefully. Above z=14 the
        // hillshade adds little detail vs the underlying topo basemap.
        Self::new(url, user_agent, 6, 14, RasterFormat::Png)
            .map(|s| s.with_attribution("© Kartverket").with_dem_halo(1))
    }

    /// Norwegian Kartverket pre-rendered shaded relief — the same WMTS
    /// cache, `layer=topograatone` (a softer grey topo that works well as
    /// a basemap beneath vector overlays). Useful when you don't have raw
    /// DEM tiles for the GPU hillshade pipeline.
    pub fn kartverket_topo_grey() -> Result<Self, reqwest::Error> {
        let user_agent = format!("turbomap/{}", env!("CARGO_PKG_VERSION"));
        Self::new(
            "https://cache.atgcp1-prod.kartverket.cloud/v1/service\
             ?layer=topograatone&style=default&tilematrixset=webmercator\
             &Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png\
             &TileMatrix={z}&TileCol={x}&TileRow={y}",
            user_agent,
            4,
            // Kartverket's topograatone WMTS pyramid only covers up to
            // z=18 — earlier the preset claimed z=20 and the demo
            // showed a cascade of 400 Bad Request warnings any time
            // the user zoomed past street level.
            18,
            RasterFormat::Png,
        )
        .map(|s| s.with_attribution("© Kartverket"))
    }

    /// Pure URL expansion. Public so tests and tools can inspect what the
    /// adapter will request without making a network call.
    pub fn url_for(&self, tile: TileId) -> String {
        expand_template(&self.url_template, tile)
    }
}

fn expand_template(template: &str, tile: TileId) -> String {
    template
        .replace("{z}", &tile.z.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string())
        .replace("{TileMatrix}", &tile.z.to_string())
        .replace("{TileCol}", &tile.x.to_string())
        .replace("{TileRow}", &tile.y.to_string())
}

impl TileSource for HttpRasterSource {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        if tile.z < self.min_zoom || tile.z > self.max_zoom {
            return Err(TileError::ZoomOutOfRange(tile.z));
        }
        let url = self.url_for(tile);
        let bytes = retry::retry(&self.retry, FetchError::is_transient, || {
            fetch_bytes(&self.client, &url)
        })
        .map_err(FetchError::into_tile_error)?;
        Ok(RasterTile {
            bytes,
            format: self.format,
        })
    }

    fn min_zoom(&self) -> u8 {
        self.min_zoom
    }

    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }

    fn raster_format(&self) -> RasterFormat {
        self.format
    }

    fn dem_halo_px(&self) -> u32 {
        self.dem_halo_px
    }
}

/// HTTP source for MVT (vector) tiles. The bytes come over the wire and are
/// decoded via `turbomap-mvt` inside `request`. The actual tessellation +
/// GPU upload happens later in the host's worker pool.
///
/// Optional disk caching of the raw protobuf bytes is enabled by setting
/// `cache_dir` — second-and-later launches read from disk and never touch
/// the network for tiles we've already seen.
#[derive(Debug, Clone)]
pub struct HttpVectorTileSource {
    client: reqwest::blocking::Client,
    url_template: String,
    min_zoom: u8,
    max_zoom: u8,
    attribution: Option<String>,
    cache: Option<DiskCache>,
    retry: RetryPolicy,
}

/// Default on-disk cache budget when a cache dir is set without an explicit
/// size: 256 MiB ≈ a city's worth of vector tiles.
pub const DEFAULT_CACHE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

impl HttpVectorTileSource {
    pub fn new(
        url_template: impl Into<String>,
        user_agent: impl Into<String>,
        min_zoom: u8,
        max_zoom: u8,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(user_agent.into())
            .timeout(Duration::from_secs(15))
            // VersaTiles and most MVT providers serve gzip-compressed PBF.
            .gzip(true)
            .build()?;
        Ok(Self {
            client,
            url_template: url_template.into(),
            min_zoom,
            max_zoom,
            attribution: None,
            cache: None,
            retry: RetryPolicy::default(),
        })
    }

    pub fn with_attribution(mut self, attribution: impl Into<String>) -> Self {
        self.attribution = Some(attribution.into());
        self
    }

    /// Override the transient-failure retry policy (default
    /// [`RetryPolicy::default`]; pass [`RetryPolicy::none`] to disable).
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// Enable a bounded on-disk cache of raw MVT bytes (default budget
    /// [`DEFAULT_CACHE_BUDGET_BYTES`]). The directory is created on first
    /// write; lookups are best-effort (any I/O error falls through to the
    /// network), and least-recently-used tiles are evicted once the budget
    /// is exceeded.
    pub fn with_cache_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.cache = Some(DiskCache::new(dir.into(), DEFAULT_CACHE_BUDGET_BYTES));
        self
    }

    /// Enable a bounded on-disk cache with an explicit byte budget.
    pub fn with_cache(mut self, dir: impl Into<std::path::PathBuf>, budget_bytes: u64) -> Self {
        self.cache = Some(DiskCache::new(dir.into(), budget_bytes));
        self
    }

    pub fn attribution(&self) -> Option<&str> {
        self.attribution.as_deref()
    }

    pub fn cache_dir(&self) -> Option<&std::path::Path> {
        self.cache.as_ref().map(|c| c.root())
    }

    /// Tile path relative to the cache root: `<z>/<x>/<y>`.
    fn tile_rel_path(tile: TileId) -> std::path::PathBuf {
        let mut p = std::path::PathBuf::from(tile.z.to_string());
        p.push(tile.x.to_string());
        p.push(tile.y.to_string());
        p
    }

    /// VersaTiles OSM (OpenStreetMap-based, OpenMapTiles schema). Free,
    /// no API key. Layer names: `water`, `transportation`, `building`,
    /// `boundary`, `place`, `landuse`, `landcover`, `waterway`, etc.
    pub fn versatiles_osm() -> Result<Self, reqwest::Error> {
        let ua = format!("turbomap/{}", env!("CARGO_PKG_VERSION"));
        Self::new(
            "https://tiles.versatiles.org/tiles/osm/{z}/{x}/{y}",
            ua,
            0,
            14,
        )
        .map(|s| s.with_attribution("© OpenStreetMap contributors / VersaTiles"))
    }

    /// Our own tileserver's multi-layer N50 basemap
    /// (`/v1/basemap/{z}/{x}/{y}.mvt`). Layer names: `water`, `glacier`,
    /// `landcover`, `building`, `coastline`, `contour`, `transportation`,
    /// `place` — styled by the document at `/v1/basemap/style.json`.
    pub fn turbo_basemap(base_url: &str) -> Result<Self, reqwest::Error> {
        let ua = format!("turbomap/{}", env!("CARGO_PKG_VERSION"));
        let base = base_url.trim_end_matches('/');
        Self::new(format!("{base}/v1/basemap/{{z}}/{{x}}/{{y}}.mvt"), ua, 4, 16)
            .map(|s| s.with_attribution("© Kartverket"))
    }

    pub fn url_for(&self, tile: TileId) -> String {
        expand_template(&self.url_template, tile)
    }
}

/// Fetch a small text document (e.g. a MapLibre `style.json`) over HTTP.
/// Blocking, like the tile sources — call from a worker/startup path, not
/// the render thread.
pub fn fetch_text(url: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("turbomap/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()?;
    client.get(url).send()?.error_for_status()?.text()
}

impl VectorTileSource for HttpVectorTileSource {
    fn request(&self, tile: TileId) -> Result<VectorTile, TileError> {
        if tile.z < self.min_zoom || tile.z > self.max_zoom {
            return Err(TileError::ZoomOutOfRange(tile.z));
        }

        // 1. Disk cache — fast path. A hit refreshes the entry's recency so
        // it survives LRU eviction. Read/decode failures drop through to the
        // network; nothing the user sees should change.
        let rel = Self::tile_rel_path(tile);
        if let Some(ref cache) = self.cache {
            if let Some(bytes) = cache.read(&rel) {
                if let Ok(decoded) = turbomap_mvt::decode(&bytes) {
                    return Ok(decoded);
                }
                log::warn!("corrupt vector cache entry for {tile:?}; refetching");
            }
        }

        // 2. Network fetch, with transient-failure retry/backoff.
        let url = self.url_for(tile);
        let bytes = retry::retry(&self.retry, FetchError::is_transient, || {
            fetch_bytes(&self.client, &url)
        })
        .map_err(FetchError::into_tile_error)?;

        // 3. Persist before decoding so a malformed tile still ends up on
        // disk (it'll be eligible for retry on the next launch). The write
        // also triggers periodic LRU eviction to keep the cache bounded.
        if let Some(ref cache) = self.cache {
            if let Err(e) = cache.write(&rel, &bytes) {
                log::warn!("vector cache write failed for {tile:?}: {e}");
            }
        }

        turbomap_mvt::decode(&bytes).map_err(|e| TileError::Decode(e.to_string()))
    }

    fn min_zoom(&self) -> u8 {
        self.min_zoom
    }

    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a developer configures a source with a template, hands
    //! it tile IDs, and expects URLs containing exactly those coordinates.
    //! The actual HTTP I/O is reqwest's responsibility and is not unit tested
    //! here — the smoke test validates the network path end-to-end.

    use super::*;

    #[test]
    fn http_status_retry_classification() {
        // Transient: server errors + request-timeout + rate-limit.
        for code in [500, 502, 503, 504, 408, 429] {
            assert!(status_is_transient(code), "{code} should be retryable");
        }
        // Permanent: client errors that won't change on retry.
        for code in [400, 401, 403, 404, 410, 451] {
            assert!(!status_is_transient(code), "{code} should NOT be retried");
        }
    }

    #[test]
    fn xyz_placeholders_are_substituted() {
        let s = HttpRasterSource::new(
            "https://tiles.example/{z}/{x}/{y}.png",
            "test/0",
            0,
            22,
            RasterFormat::Png,
        )
        .unwrap();
        let url = s.url_for(TileId::new(11, 1054, 706));
        assert_eq!(url, "https://tiles.example/11/1054/706.png");
    }

    #[test]
    fn wmts_placeholders_are_substituted() {
        let s = HttpRasterSource::new(
            "https://example.test?TileMatrix={TileMatrix}&TileCol={TileCol}&TileRow={TileRow}",
            "test/0",
            0,
            22,
            RasterFormat::Png,
        )
        .unwrap();
        let url = s.url_for(TileId::new(7, 65, 33));
        assert_eq!(
            url,
            "https://example.test?TileMatrix=7&TileCol=65&TileRow=33",
        );
    }

    #[test]
    fn kartverket_preset_uses_published_endpoint_and_format() {
        let s = HttpRasterSource::kartverket_topo().unwrap();
        let url = s.url_for(TileId::new(11, 1054, 706));
        // The exact Kartverket WMTS endpoint, with our coordinates filled in.
        assert!(url.contains("cache.atgcp1-prod.kartverket.cloud"));
        assert!(url.contains("layer=topo"));
        assert!(url.contains("tilematrixset=webmercator"));
        assert!(url.contains("Format=image/png"));
        assert!(url.contains("TileMatrix=11"));
        assert!(url.contains("TileCol=1054"));
        assert!(url.contains("TileRow=706"));
        assert_eq!(s.min_zoom(), 4);
        assert_eq!(s.max_zoom(), 20);
        assert_eq!(s.attribution(), Some("© Kartverket"));
    }

    #[test]
    fn out_of_range_zoom_is_rejected_without_network_io() {
        // The trait is well-typed enough that the adapter can refuse out-of
        // -range zooms before hitting the network. Hosts rely on this so
        // their fetch threads don't fire pointless requests.
        let s = HttpRasterSource::new(
            "http://127.0.0.1:0/{z}/{x}/{y}", // unbindable port; any request would fail
            "test/0",
            5,
            10,
            RasterFormat::Png,
        )
        .unwrap();
        let err = s.request(TileId::new(2, 0, 0)).unwrap_err();
        assert!(matches!(err, TileError::ZoomOutOfRange(2)), "got {:?}", err,);
    }

    // ---- vector disk cache ---------------------------------------------
    //
    // Value boundary: when `with_cache_dir` is set, a request that finds
    // bytes on disk returns the decoded tile *without touching the
    // network*. The integration test below uses an unreachable URL — if
    // the source ever tried to fetch, the test would hang or fail.

    use tempfile::TempDir;

    /// Empty MVT protobuf — decodes as a VectorTile with zero layers. Good
    /// enough for "did the cache path get used" assertions without needing
    /// to hand-encode a real tile.
    const EMPTY_MVT: &[u8] = &[];

    #[test]
    fn vector_cache_serves_a_tile_present_on_disk_without_network() {
        let dir = TempDir::new().unwrap();
        // Pre-seed the cache at <dir>/11/1054/590.
        let tile = TileId::new(11, 1054, 590);
        let path = dir
            .path()
            .join(tile.z.to_string())
            .join(tile.x.to_string())
            .join(tile.y.to_string());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, EMPTY_MVT).unwrap();

        // Source URL points at a blackhole — if the cache lookup misses,
        // the request will hang on the timeout, not return cleanly. So a
        // fast successful return = the cache served the tile.
        let source = HttpVectorTileSource::new(
            "http://10.255.255.1:81/{z}/{x}/{y}", // RFC 5737 blackhole-ish
            "test/0",
            0,
            22,
        )
        .unwrap()
        .with_cache_dir(dir.path());

        let decoded = source.request(tile).expect("cache hit must succeed");
        assert_eq!(decoded.layers.len(), 0, "EMPTY_MVT decodes to 0 layers");
    }

    #[test]
    fn vector_cache_writes_back_after_a_network_fetch_completes() {
        // We can't exercise the network end-to-end without mocking HTTP, but
        // the writeback path is `DiskCache::write` at the source's `<z>/<x>/<y>`
        // relative path — assert that lands a readable entry at the expected
        // absolute location.
        let dir = TempDir::new().unwrap();
        let tile = TileId::new(7, 64, 32);
        let cache = DiskCache::new(dir.path(), DEFAULT_CACHE_BUDGET_BYTES);
        cache
            .write(&HttpVectorTileSource::tile_rel_path(tile), b"hello")
            .unwrap();
        let read_back = std::fs::read(
            dir.path().join("7").join("64").join("32"),
        )
        .unwrap();
        assert_eq!(&read_back, b"hello");
    }

    #[test]
    fn vector_cache_path_layout_is_z_x_y() {
        let rel = HttpVectorTileSource::tile_rel_path(TileId::new(7, 64, 32));
        assert_eq!(
            rel.to_string_lossy(),
            format!("7{0}64{0}32", std::path::MAIN_SEPARATOR)
        );
    }

    #[test]
    fn cache_dir_is_none_until_configured() {
        let source = HttpVectorTileSource::new("u", "a", 0, 22).unwrap();
        assert!(source.cache_dir().is_none());
        let dir = TempDir::new().unwrap();
        let cached = source.with_cache_dir(dir.path());
        assert_eq!(cached.cache_dir(), Some(dir.path()));
    }
}
