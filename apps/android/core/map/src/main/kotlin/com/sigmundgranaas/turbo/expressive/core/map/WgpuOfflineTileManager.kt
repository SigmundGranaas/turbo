package com.sigmundgranaas.turbo.expressive.core.map

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TURBOMAP_TILE_DIR
import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TileStore
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
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
import kotlinx.coroutines.withContext
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
import kotlin.random.Random

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
    /**
     * Fetches the routing pack for a region. `null` disables routing
     * downloads entirely — which is what the tile-focused tests want, and
     * a legitimate configuration rather than a test affordance: a build
     * without the routing module still browses maps.
     */
    private val packs: PackDownloader? = null,
    /**
     * Cut the pack here when the server has none. `null` — the default —
     * means a region without a server-side pack simply completes
     * without offline routing, which is the behaviour that shipped.
     *
     * A lambda and not a type because the implementation lives in
     * `:core:routing-android`, which depends on this module. Inverting
     * that would put the FFI, JNA and the whole native library into the
     * dependencies of every build that draws a map.
     */
    private val deviceBuild: (suspend (DownloadSpec, (Float) -> Unit) -> DeviceBuildResult)? = null,
) : OfflineTileManager {

    /** What a device-side build came back with. */
    sealed interface DeviceBuildResult {
        data class Done(val bytes: Long) : DeviceBuildResult

        /** Not attempted — the user has not turned device builds on. */
        data object Disabled : DeviceBuildResult

        /**
         * The user stopped it: paused, deleted, or the network policy
         * withdrew.
         *
         * Distinct from [Disabled], which it used to be folded into.
         * `Disabled` means "this region gets no routing, carry on", so a
         * cancelled build read as permission to finish the region and
         * mark it Complete without routing data. That it did not happen
         * relied on the surrounding coroutine also being cancelled and
         * throwing at the next suspension point — true today, but a
         * property of the caller rather than anything stated here.
         */
        data object Cancelled : DeviceBuildResult

        data class Failed(val reason: String) : DeviceBuildResult
    }

    @Inject
    constructor(
        @ApplicationContext context: Context,
        serviceLauncher: OfflineServiceLauncher,
        packSource: com.sigmundgranaas.turbo.expressive.domain.PackSource,
        devicePacks: DevicePackBuild,
        // Shared, not built here. Tiles and packs are fetched from the
        // same hosts, and two clients meant two pools and no reused
        // sockets between the lanes of a single download.
        http: OkHttpClient,
    ) : this(
        tileStore = TileStore(File(context.cacheDir, TURBOMAP_TILE_DIR)),
        store = OfflineRegionStore(File(context.filesDir, REGION_META_DIR)),
        serviceLauncher = serviceLauncher,
        fetcher = okHttpFetcher(tileClient(http)),
        laneProvider = ::defaultLanes,
        scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
        now = System::currentTimeMillis,
        packs = PackDownloader(
            root = File(context.filesDir, RoutingPack.DIR),
            source = packSource::current,
            fetch = okHttpPackFetcher(tileClient(http)),
        ),
        deviceBuild = devicePacks::build,
    )

    /**
     * The device-build seam, as an injectable type.
     *
     * `:core:routing-android` provides the real one. A build without
     * that module gets [DevicePackBuild.Disabled], so the map still
     * downloads — the routing library is optional and this keeps it so.
     */
    interface DevicePackBuild {
        suspend fun build(spec: DownloadSpec, onProgress: (Float) -> Unit): DeviceBuildResult

        /** No device builds; regions complete without offline routing. */
        object Disabled : DevicePackBuild {
            override suspend fun build(
                spec: DownloadSpec,
                onProgress: (Float) -> Unit,
            ): DeviceBuildResult = DeviceBuildResult.Disabled
        }
    }

    /**
     * Run the device build, if one is configured, reporting into the
     * same slice of the progress bar the download would have used.
     */
    private suspend fun buildOnDevice(
        id: Long,
        spec: DownloadSpec,
        packShare: Double,
    ): DeviceBuildResult {
        val build = deviceBuild ?: return DeviceBuildResult.Disabled
        return build(spec) { f ->
            update(id) { it.copy(progress = (f * packShare).toFloat()) }
        }
    }

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

    // Cancel, but leave the entry in `jobs`. A cancelled job is not a
    // stopped job — it keeps running until it reaches a suspension
    // point, and a device pack build has none for minutes. Dropping the
    // reference here would leave a later `resume` or `retry` with
    // nothing to join, which is the same double-start `startJob` exists
    // to prevent. The job removes its own entry when it finally unwinds.
    override fun pause(id: Long) {
        userPaused += id
        jobs[id]?.cancel()
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
        _regions.value.filter {
            // Building counts. It is the phase that fetches the most on
            // someone else's data plan, so leaving it out would mean
            // "Wi-Fi only" silently did not apply to the longest and
            // heaviest part of a download.
            it.status == OfflineStatus.Downloading || it.status == OfflineStatus.Building
        }.forEach {
            // Kept, not removed — see [pause].
            jobs[it.id]?.cancel()
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
        jobs[id]?.cancel()
        userPaused.remove(id)
        val spec = specs.remove(id)
        // One region, one delete. Leaving the pack behind would leave
        // megabytes the user cannot see in any list and cannot remove.
        spec?.takeIf { it.includeRouting }?.let { packs?.delete(it.bounds) }
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

    /**
     * Start (or restart) the job for one region.
     *
     * **The previous job is joined, not merely cancelled.** `Job.cancel()`
     * requests cancellation and returns; the coroutine keeps running until
     * it reaches a suspension point, and a device pack build has none for
     * minutes at a time — it is a blocking call into native code. Every
     * caller here can fire while a job is live: [retry], [resume],
     * [add], and [resumeNetworkPaused], which runs on its own when Wi-Fi
     * comes back. Launching without joining therefore ran two jobs for the
     * same region at once, both writing the same tile files and the same
     * pack directory.
     *
     * That race is where three separate user-visible failures came from:
     * a build whose working directory was deleted by its own replacement,
     * a cancelled build that kept running while reporting nothing, and a
     * retry queued behind it. Joining removes the class rather than the
     * symptoms.
     */
    private fun startJob(id: Long, spec: DownloadSpec) {
        val previous = jobs.remove(id)
        previous?.cancel()
        jobs[id] = scope.launch {
            // Inside the new job, so the caller is not blocked: `startJob`
            // is called from the UI thread. The wait is bounded by how
            // long the old job takes to notice — for a pack build, by the
            // request it is in the middle of.
            previous?.join()
            // After the join, never before: the job being replaced marks
            // the region Paused on its way out, and setting Downloading
            // first would let that land afterwards and leave a running
            // download presenting as paused.
            update(id) { it.copy(status = OfflineStatus.Downloading, errorReason = null) }
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

        // The routing pack first, and its share of the progress bar is
        // its share of the BYTES, not of the file count. Five files
        // against a thousand tiles would round to nothing by count while
        // being a third of the download — a bar that sits at 0% through
        // the slowest part and then races.
        val estimate = TileMath.estimate(spec)
        val packShare = if (spec.includeRouting && estimate.bytes > 0) {
            (estimate.packBytes.toDouble() / estimate.bytes).coerceIn(0.0, 0.9)
        } else {
            0.0
        }
        var packBytes = 0L
        if (spec.includeRouting && packs != null) {
            when (val r = packs.download(
                bounds = spec.bounds,
                onProgress = { soFar, packTotal ->
                    if (packTotal > 0) {
                        val f = (soFar.toDouble() / packTotal * packShare).toFloat()
                        update(id) { it.copy(progress = f) }
                    }
                },
                waitForBuild = { seconds -> delay(seconds.coerceIn(1, 60) * 1000L) },
            )) {
                is PackDownloader.Outcome.Done -> packBytes = r.bytes
                // This server has no pack endpoint yet. The app and the
                // tileserver ship separately, so there is a window in
                // whichever order they land where one asks and the other
                // has never heard of packs. Failing here would break
                // offline downloads — which work today — for everyone in
                // that window. The region completes without routing data;
                // the next download after the server ships gets it.
                //
                // Unless the phone can cut one itself. That is off by
                // default and stays off unless the user asked for it:
                // it is minutes of work and tens of megabytes from
                // Kartverket, which is not a thing to start because a
                // map download found a server without a pack endpoint.
                PackDownloader.Outcome.Unsupported -> {
                    when (val b = buildOnDevice(id, spec, packShare)) {
                        is DeviceBuildResult.Done -> packBytes = b.bytes
                        DeviceBuildResult.Disabled -> Unit
                        // Stop the whole region, do not carry on into the
                        // tiles. The user asked for this to stop, and
                        // finishing the download would mark it Complete —
                        // a region that browses but cannot route, which
                        // they have no way to see and no way to repair
                        // short of deleting and starting again.
                        DeviceBuildResult.Cancelled -> {
                            markPaused(id)
                            return
                        }
                        // A build the user explicitly asked for and did
                        // not get is worth failing the region over —
                        // unlike the server having no packs, this is not
                        // a deployment window, it is the thing they
                        // turned on not working.
                        is DeviceBuildResult.Failed -> {
                            markFailed(id, b.reason)
                            return
                        }
                    }
                }
                // Too big for one pack, but not too big for tiles — the
                // two caps are different sizes because a tile pyramid
                // degrades as it grows and a pack does not. The dialog
                // already told the user this area comes without offline
                // routing; failing the download now would take the map
                // away too, which is not what they asked for.
                is PackDownloader.Outcome.TooLarge -> Unit
                // A pack the server DOES serve and could not deliver is a
                // real failure. The alternative — a region that browses
                // but cannot route — is a state the user has no way to
                // see and no way to fix, and it would surface much later
                // as "why does routing work over there and not here".
                is PackDownloader.Outcome.Failed -> {
                    markFailed(id, r.reason)
                    return
                }
            }
        }

        if (total == 0) {
            update(id) {
                it.copy(status = OfflineStatus.Complete, progress = 1f, sizeBytes = packBytes)
            }
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
                                        progress = (packShare + (1.0 - packShare) *
                                            (done.toDouble() / total)).toFloat(),
                                        tileCount = stored.toLong(),
                                        sizeBytes = bytes + packBytes,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        // A few tiles short is not a failed download.
        //
        // This used to fail the region if a single tile errored, which in
        // practice meant a 2000-tile area over a rate-limiting public WMTS
        // reached 90-odd percent and then threw all of it away — the tiles
        // were on disk and usable, and the screen said "Download failed".
        // A missing tile degrades to a gap in one corner of one zoom level;
        // it is not remotely the same event as a download that did not
        // happen, and conflating them trained the user to distrust a
        // feature that was mostly working.
        //
        // So: fail only when enough failed that the area genuinely is not
        // covered. Short of that, the region completes and carries a note
        // saying what is missing. Retry re-runs it, and because stored
        // tiles are skipped it costs only the gaps.
        val ratio = if (total > 0) failed.toDouble() / total else 0.0
        when {
            ratio > MAX_MISSING_RATIO ->
                markFailed(id, "$failed of $total tiles could not be downloaded")

            else -> update(id) {
                it.copy(
                    status = OfflineStatus.Complete,
                    progress = 1f,
                    tileCount = stored.toLong(),
                    sizeBytes = bytes + packBytes,
                    errorReason = if (failed > 0) "$failed of $total tiles are missing" else null,
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
                // Exponential, and jittered. A fixed delay put all six
                // workers back on the wire at the same instant, so three
                // attempts were really one attempt against a server that
                // was still shedding load — which is how a download loses
                // a hundred tiles to a burst that lasted two seconds.
                FetchOutcome.Error -> if (attempt < FETCH_RETRIES - 1) {
                    val backoff = RETRY_BACKOFF_MS shl attempt
                    delay(backoff + Random.nextLong(backoff / 2))
                }
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
        /** Five tries with exponential backoff spans ~12s — long enough to
         *  outlast the load bursts that were costing whole downloads. */
        private const val FETCH_RETRIES = 5
        private const val RETRY_BACKOFF_MS = 400L

        /**
         * How much of a region may be missing and still count as
         * downloaded. Five percent of a tile pyramid is a gap the user has
         * to hunt for; the alternative on offer was discarding the other
         * ninety-five.
         */
        private const val MAX_MISSING_RATIO = 0.05
        private const val MAX_TILE_BYTES = 8L * 1024 * 1024
        private const val TIMEOUT_MS = 10_000L
        private const val USER_AGENT = "turbo-android-wgpu"

        /**
         * Where routing packs are served. No data version in the path —
         * the edge cache prefixes its own, so a rebuild orphans packs
         * exactly as it orphans tiles, and a client that had to know the
         * server's version first would need a round trip to learn a
         * string.
         */

        /** `202`: the region is still being cut. Not a failure. */
        private const val HTTP_ACCEPTED = 202
        private const val DEFAULT_RETRY_AFTER = 10

        /** `404` on the manifest: this server serves no packs. */
        private const val HTTP_NOT_FOUND = 404

        /**
         * What the pack endpoint answers for a region past its cap.
         *
         * Mapped separately from the other 4xx because the region is
         * not broken, it is big — see [PackDownloader.Outcome.TooLarge].
         */
        private const val HTTP_BAD_REQUEST = 400

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

        /**
         * Streams one pack file to disk.
         *
         * Streamed rather than `bytes()` because the DEM is megabytes and
         * this runs on a phone: buffering a whole artifact to write it out
         * again spends the memory the file was going to occupy anyway,
         * twice, on the device least able to spare it.
         *
         * `202` is not an error — the server is still cutting the region
         * and says when to come back.
         */
        private fun okHttpPackFetcher(
            http: OkHttpClient,
        ): suspend (String, File) -> PackDownloader.FetchResult = { url, into ->
            withContext(Dispatchers.IO) {
                try {
                    http.newCall(Request.Builder().url(url).header("User-Agent", USER_AGENT).build())
                        .execute()
                        .use { resp ->
                            when {
                                resp.code == HTTP_ACCEPTED -> PackDownloader.FetchResult.Building(
                                    resp.header("Retry-After")?.toIntOrNull() ?: DEFAULT_RETRY_AFTER,
                                )
                                resp.code == HTTP_NOT_FOUND -> PackDownloader.FetchResult.NotFound
                                resp.code == HTTP_BAD_REQUEST -> PackDownloader.FetchResult.TooLarge
                                !resp.isSuccessful ->
                                    PackDownloader.FetchResult.Failed("Server said ${resp.code}.")
                                else -> {
                                    val body = resp.body
                                        ?: return@use PackDownloader.FetchResult.Failed("Empty response.")
                                    into.parentFile?.mkdirs()
                                    body.byteStream().use { input ->
                                        into.outputStream().use { output -> input.copyTo(output) }
                                    }
                                    PackDownloader.FetchResult.Ok(into.length())
                                }
                            }
                        }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    PackDownloader.FetchResult.Failed(e.message ?: "Download failed")
                }
            }
        }

        /**
         * The shared client with tile-sized patience.
         *
         * `newBuilder` keeps the pool, dispatcher and any interceptors —
         * this is the same client with different timeouts, not a second
         * one. Ten seconds is right for a tile and wrong for anything
         * large, which is why the pack builder sets its own.
         */
        private fun tileClient(base: OkHttpClient): OkHttpClient = base.newBuilder()
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
