package com.sigmundgranaas.turbo.expressive.core.map

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TURBOMAP_TILE_DIR
import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TileStore
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.ui.map.MapStyles
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.withPermit
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.coroutineContext

/**
 * Non-MapLibre offline downloader. Pre-populates the SAME on-disk [TileStore] the
 * wgpu map serves tiles from (dir [TURBOMAP_TILE_DIR]), so a downloaded region
 * renders with zero network — that read-through cache *is* the offline store.
 *
 * For each region it enumerates the exact `(layer, z, x, y)` the map would request
 * — the base + overlay raster lanes from [MapStyles.turbomapRasterSpecs], with the
 * identical URL templates — fetches each tile over OkHttp into the store, and
 * streams progress. Region metadata (name / extent / zoom span / status) is
 * persisted by [OfflineRegionStore] so the list survives relaunch. Downloads run
 * as per-region coroutine jobs; pause/network-gating cancel the job and keep the
 * tiles already fetched, so resume/retry re-runs and skips what's on disk.
 *
 * (DEM + vector-water lanes — needed for fully-offline 3D/water — are a documented
 * follow-up; this ships raster parity with the MapLibre downloader it replaces.)
 */
@Singleton
class WgpuOfflineTileManager internal constructor(
    private val tileStore: TileStore,
    private val store: OfflineRegionStore,
    private val serviceLauncher: OfflineServiceLauncher,
    private val fetcher: suspend (String) -> FetchOutcome,
    private val laneProvider: (DownloadSpec) -> List<Lane>,
    private val scope: CoroutineScope,
    private val now: () -> Long,
) : OfflineTileManager {

    @Inject
    constructor(
        @ApplicationContext context: Context,
        serviceLauncher: OfflineServiceLauncher,
    ) : this(
        tileStore = TileStore(File(context.cacheDir, TURBOMAP_TILE_DIR)),
        store = OfflineRegionStore(File(context.filesDir, REGION_META_DIR)),
        serviceLauncher = serviceLauncher,
        fetcher = okHttpFetcher(defaultHttp()),
        laneProvider = ::defaultLanes,
        scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
        now = System::currentTimeMillis,
    )

    /** One source's tile pyramid: a cache-key layer id + URL template + native max zoom. */
    data class Lane(val layer: String, val urlTemplate: String, val maxZoom: Int)

    /** Outcome of fetching one tile. [Absent] (404 / empty body) is legitimate —
     *  a water tile over land, an ocean DEM tile — and must not fail a region;
     *  only [Error] (transient/server failure) is retried and counts as a failure. */
    sealed interface FetchOutcome {
        data class Data(val bytes: ByteArray) : FetchOutcome
        data object Absent : FetchOutcome
        data object Error : FetchOutcome
    }

    private val _regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override val regions: StateFlow<List<OfflineRegionInfo>> = _regions.asStateFlow()

    /** The spec each region was created from — needed to re-enumerate on resume/delete. */
    private val specs = ConcurrentHashMap<Long, DownloadSpec>()
    private val jobs = ConcurrentHashMap<Long, Job>()
    private val userPaused = ConcurrentHashMap.newKeySet<Long>()
    @Volatile private var networkAllowed = true
    private val nextId = AtomicLong(1L)

    init {
        // Restore persisted regions. A region left mid-download (the process died)
        // has no live job, so present it Paused — the user (or the network policy)
        // can resume it, which re-enumerates and skips the tiles already on disk.
        val loaded = store.loadAll().map {
            if (it.status == OfflineStatus.Downloading) it.copy(status = OfflineStatus.Paused) else it
        }
        loaded.forEach { if (it.bounds != null) specs[it.id] = it.toSpec() }
        nextId.set((loaded.maxOfOrNull { it.id } ?: 0L) + 1L)
        _regions.value = loaded.sortedBy { it.name }
    }

    override fun estimate(spec: DownloadSpec): OfflineEstimate = TileMath.estimate(spec)

    override fun refresh() = Unit // state is authoritative in memory + on disk

    override fun download(spec: DownloadSpec) {
        if (!TileMath.isWithinLimits(spec)) {
            persist(newRegion(nextId.getAndIncrement(), spec, OfflineStatus.Failed, error = "Area too large"))
            return
        }
        val id = nextId.getAndIncrement()
        specs[id] = spec
        serviceLauncher.ensureRunning()
        persist(newRegion(id, spec, OfflineStatus.Downloading))
        if (networkAllowed) startJob(id, spec) else markPaused(id)
    }

    override fun retry(id: Long) {
        val spec = specs[id] ?: return
        userPaused.remove(id)
        serviceLauncher.ensureRunning()
        if (networkAllowed) startJob(id, spec) else markPaused(id)
    }

    override fun pause(id: Long) {
        userPaused += id
        jobs.remove(id)?.cancel()
        markPaused(id)
    }

    override fun resume(id: Long) {
        val spec = specs[id] ?: return
        userPaused.remove(id)
        serviceLauncher.ensureRunning()
        if (networkAllowed) startJob(id, spec)
    }

    override fun setNetworkAllowed(allowed: Boolean) {
        networkAllowed = allowed
        if (!allowed) pauseActiveForNetwork() else resumeNetworkPaused()
    }

    /** Network lost: pause everything still in flight (keep tiles). */
    private fun pauseActiveForNetwork() {
        _regions.value.filter { it.status == OfflineStatus.Downloading }.forEach {
            jobs.remove(it.id)?.cancel()
            markPaused(it.id)
        }
    }

    /** Network back: resume regions the user didn't pause explicitly. */
    private fun resumeNetworkPaused() {
        _regions.value
            .filter { it.status == OfflineStatus.Paused && it.id !in userPaused }
            .forEach { r -> specs[r.id]?.let { startJob(r.id, it) } }
    }

    override fun rename(id: Long, name: String) {
        update(id) { it.copy(name = name) }
    }

    override fun delete(id: Long) {
        jobs.remove(id)?.cancel()
        userPaused.remove(id)
        val spec = specs.remove(id)
        store.delete(id)
        _regions.update { list -> list.filterNot { it.id == id } }
        // Drop this region's tiles, but keep any still covered by another region.
        if (spec != null) {
            val mine = tileKeys(spec)
            val others = specs.values.flatMap { tileKeys(it) }.toHashSet()
            mine.filterNot { it in others }.forEach { it.delete() }
        }
    }

    override fun clearAmbientCache() {
        // Ambient = anything in the store not part of a downloaded region. Keep the
        // union of every region's tiles; prune the rest.
        val keep = specs.values.flatMap { tileKeys(it) }.toHashSet()
        tileStore.pruneExcept(keep)
    }

    // ---- download orchestration -------------------------------------------------

    private fun startJob(id: Long, spec: DownloadSpec) {
        jobs.remove(id)?.cancel()
        update(id) { it.copy(status = OfflineStatus.Downloading, errorReason = null) }
        jobs[id] = scope.launch {
            try {
                runDownload(id, spec)
            } catch (c: CancellationException) {
                markPaused(id) // paused by user or network — partial tiles remain
                throw c
            } catch (e: Exception) {
                markFailed(id, e.message ?: "Download failed")
            } finally {
                jobs.remove(id, coroutineContext[Job])
            }
        }
    }

    private suspend fun runDownload(id: Long, spec: DownloadSpec) {
        val work = laneProvider(spec).flatMap { lane ->
            val hi = minOf(spec.maxZoom, lane.maxZoom.toDouble())
            TileMath.tilesFor(spec.bounds, spec.minZoom, hi).map { lane to it }
        }
        val total = work.size
        if (total == 0) {
            update(id) { it.copy(status = OfflineStatus.Complete, progress = 1f) }
            return
        }
        val sem = Semaphore(PARALLELISM)
        val lock = Mutex()
        var done = 0
        var stored = 0
        var bytes = 0L
        var failed = 0
        coroutineScope {
            work.forEach { (lane, t) ->
                launch {
                    sem.withPermit {
                        coroutineContext.ensureActive()
                        // An absent tile (404 / empty body) is legitimate — a water lane
                        // over land, or a DEM tile over open sea, simply has no data — so
                        // it must NOT fail the region. Only a real fetch error does.
                        var errored = false
                        if (!tileStore.exists(lane.layer, t.z, t.x, t.y)) {
                            when (val r = fetch(urlFor(lane, t))) {
                                is FetchOutcome.Data -> tileStore.put(lane.layer, t.z, t.x, t.y, r.bytes)
                                FetchOutcome.Absent -> Unit
                                FetchOutcome.Error -> errored = true
                            }
                        }
                        val size = tileStore.size(lane.layer, t.z, t.x, t.y)
                        lock.withLock {
                            done++
                            bytes += size
                            if (size > 0L) stored++
                            if (errored) failed++
                            if (done % PROGRESS_EVERY == 0 || done == total) {
                                update(id) {
                                    it.copy(
                                        progress = done.toFloat() / total,
                                        tileCount = stored.toLong(),
                                        sizeBytes = bytes,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        if (failed > 0) {
            markFailed(id, "$failed of $total tiles could not be downloaded")
        } else {
            update(id) {
                it.copy(
                    status = OfflineStatus.Complete,
                    progress = 1f,
                    tileCount = stored.toLong(),
                    sizeBytes = bytes,
                )
            }
        }
    }

    /** Fetch one tile, retrying only transient [FetchOutcome.Error]s; an
     *  [FetchOutcome.Absent] (no data here) returns immediately, not as a failure. */
    private suspend fun fetch(url: String): FetchOutcome {
        repeat(FETCH_RETRIES) { attempt ->
            coroutineContext.ensureActive()
            val outcome = try {
                fetcher(url)
            } catch (c: CancellationException) {
                throw c // a pause/network-gate cancellation must propagate, not become an Error
            } catch (e: Exception) {
                FetchOutcome.Error
            }
            when (outcome) {
                is FetchOutcome.Data, FetchOutcome.Absent -> return outcome
                FetchOutcome.Error -> if (attempt < FETCH_RETRIES - 1) delay(RETRY_BACKOFF_MS)
            }
        }
        return FetchOutcome.Error
    }

    private fun urlFor(lane: Lane, t: TileMath.TileXyz): String =
        lane.urlTemplate.replace("{z}", "${t.z}").replace("{x}", "${t.x}").replace("{y}", "${t.y}")

    /** Every cache file a region's tiles occupy — for delete/prune set math. */
    private fun tileKeys(spec: DownloadSpec): List<File> =
        laneProvider(spec).flatMap { lane ->
            val hi = minOf(spec.maxZoom, lane.maxZoom.toDouble())
            TileMath.tilesFor(spec.bounds, spec.minZoom, hi).map { tileStore.fileOf(lane.layer, it.z, it.x, it.y) }
        }

    // ---- state helpers ----------------------------------------------------------

    private fun newRegion(id: Long, spec: DownloadSpec, status: OfflineStatus, error: String? = null) =
        OfflineRegionInfo(
            id = id,
            name = spec.name,
            status = status,
            progress = 0f,
            sizeBytes = 0L,
            tileCount = 0L,
            base = spec.base,
            overlays = spec.overlays,
            bounds = spec.bounds,
            minZoom = spec.minZoom,
            maxZoom = spec.maxZoom,
            createdAtEpochMs = now(),
            errorReason = error,
        )

    private fun markPaused(id: Long) = update(id) {
        if (it.status == OfflineStatus.Complete) it else it.copy(status = OfflineStatus.Paused)
    }

    private fun markFailed(id: Long, reason: String) =
        update(id) { it.copy(status = OfflineStatus.Failed, errorReason = reason) }

    /** Apply [transform] to the region (if present), publish + persist atomically. */
    private fun update(id: Long, transform: (OfflineRegionInfo) -> OfflineRegionInfo) {
        if (!specs.containsKey(id)) return // deleted out from under an in-flight job
        var updated: OfflineRegionInfo? = null
        _regions.update { list ->
            list.map { if (it.id == id) transform(it).also { u -> updated = u } else it }
        }
        updated?.let { store.save(it) }
    }

    private fun persist(info: OfflineRegionInfo) {
        _regions.update { list -> (list.filterNot { it.id == info.id } + info).sortedBy { it.name } }
        store.save(info)
    }

    private fun OfflineRegionInfo.toSpec(): DownloadSpec =
        DownloadSpec(name, base, bounds ?: error("region without bounds"), minZoom, maxZoom, overlays)

    companion object {
        private const val REGION_META_DIR = "offline-regions"
        private const val PARALLELISM = 6
        private const val PROGRESS_EVERY = 16
        private const val FETCH_RETRIES = 3
        private const val RETRY_BACKOFF_MS = 400L
        private const val MAX_TILE_BYTES = 8L * 1024 * 1024
        private const val TIMEOUT_MS = 10_000L
        private const val USER_AGENT = "turbo-android-wgpu"

        /** Cache-key layer for DEM tiles — matches turbomap-ffi's TERRAIN_KEY and
         *  TurbomapMapView.isDemKey, so a pre-populated DEM tile hits at render. */
        private const val DEM_LAYER = "__terrain"

        /** Norway's DEM is ~10 m native (≈ z14); finer requests just upsample, and
         *  the engine over-zooms a shallow DEM ("deep zooms drape on a shallow DEM",
         *  render/terrain.rs), so capping offline DEM here gives full relief fidelity
         *  at a fraction of the tiles of the raster max. */
        private const val DEM_MAX_ZOOM = 14

        /** Every tile lane the wgpu map requests for a region: base + overlay
         *  rasters, the vector-water basemap, and the DEM heightmap (3D). Identical
         *  layer ids + URL templates the map fetches, so offline tiles hit at render. */
        private fun defaultLanes(spec: DownloadSpec): List<Lane> {
            val raster = MapStyles.turbomapRasterSpecs(spec.base, spec.overlays)
                .map { Lane(it.id, it.tileUrlTemplate, it.maxZoom) }
            val vector = MapStyles.turbomapVectorSpecs()
                .map { Lane(it.id, it.tileUrlTemplate, it.maxZoom) }
            val dem = Lane(DEM_LAYER, MapStyles.TERRAIN_DEM_URL, DEM_MAX_ZOOM)
            return raster + vector + dem
        }

        private fun defaultHttp(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .readTimeout(TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .build()

        /** A single-shot tile GET over [http], mapped to a [FetchOutcome]: 404/empty
         *  → Absent (no data here); non-OK / oversize / null body → Error; else Data. */
        private fun okHttpFetcher(http: OkHttpClient): suspend (String) -> FetchOutcome = { url ->
            http.newCall(Request.Builder().url(url).header("User-Agent", USER_AGENT).build())
                .execute()
                .use { r ->
                    when {
                        r.code == 404 || r.code == 204 || r.code == 410 -> FetchOutcome.Absent
                        !r.isSuccessful -> FetchOutcome.Error
                        (r.body?.contentLength() ?: -1L) > MAX_TILE_BYTES -> FetchOutcome.Error
                        else -> {
                            val bytes = r.body?.bytes()
                            when {
                                bytes == null -> FetchOutcome.Error
                                bytes.isEmpty() -> FetchOutcome.Absent
                                bytes.size > MAX_TILE_BYTES -> FetchOutcome.Error
                                else -> FetchOutcome.Data(bytes)
                            }
                        }
                    }
                }
        }
    }
}
