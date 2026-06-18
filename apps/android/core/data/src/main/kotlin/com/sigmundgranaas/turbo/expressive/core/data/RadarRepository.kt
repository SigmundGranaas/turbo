package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RadarGridFrame
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import java.time.Instant
import javax.inject.Inject

/**
 * Gridded near-term weather for the cloud overlay. Returns a short sequence of
 * [RadarGridFrame]s — cloud coverage + precipitation over the viewport, one per
 * forecast hour — that the overlay crossfades and a time slider scrubs.
 */
interface RadarRepository {
    /**
     * [frames] timesteps of coverage/precip across [bounds]. May return fewer
     * (or empty) if the upstream is unavailable; callers fall back accordingly.
     */
    suspend fun forecastFrames(bounds: GeoBounds, frames: Int): List<RadarGridFrame>
}

/**
 * MET Norway live path: samples `locationforecast/2.0` on a coarse lat/lng grid
 * over the viewport (one full timeseries per point, fetched in parallel), then
 * resamples `cloud_area_fraction` + `precipitation_amount` to the overlay grid.
 *
 * The grid is intentionally coarse ([SAMPLE_COLS]×[SAMPLE_ROWS]) — it's only a
 * smooth regional *seed*; the GPU overlay grows all the visible cloud detail
 * procedurally. Each sample's lat/lng is known, so the result is geo-registered
 * by construction (no image reprojection). The shared client's `HttpCache`
 * dedupes repeat point fetches, keeping us within MET's fair-use policy.
 */
class HttpRadarRepository @Inject constructor(
    private val client: HttpClient,
) : RadarRepository {

    override suspend fun forecastFrames(bounds: GeoBounds, frames: Int): List<RadarGridFrame> {
        val n = frames.coerceIn(1, 24)
        // Sample points row-major: r=0 is north (top), c=0 is west (left).
        val points = ArrayList<Pair<Double, Double>>(SAMPLE_COLS * SAMPLE_ROWS)
        for (r in 0 until SAMPLE_ROWS) {
            val lat = lerp(bounds.north, bounds.south, frac(r, SAMPLE_ROWS))
            for (c in 0 until SAMPLE_COLS) {
                val lng = lerp(bounds.west, bounds.east, frac(c, SAMPLE_COLS))
                points.add(lat to lng)
            }
        }
        val perPoint: List<List<SampleStep>> = coroutineScope {
            points.map { (lat, lng) -> async { fetchPoint(lat, lng, n) } }.awaitAll()
        }
        if (perPoint.any { it.isEmpty() }) return emptyList()
        return buildFrames(perPoint, SAMPLE_COLS, SAMPLE_ROWS, GRID_W, GRID_H, n)
    }

    private suspend fun fetchPoint(lat: Double, lng: Double, frames: Int): List<SampleStep> {
        val res: Lf = client
            .get("https://api.met.no/weatherapi/locationforecast/2.0/compact") {
                parameter("lat", "%.3f".format(lat))
                parameter("lon", "%.3f".format(lng))
                header("User-Agent", USER_AGENT)
            }
            .body()
        return res.properties?.timeseries.orEmpty().take(frames).mapNotNull { s ->
            val time = s.time ?: return@mapNotNull null
            SampleStep(
                epochMillis = runCatching { Instant.parse(time).toEpochMilli() }.getOrDefault(0L),
                coverage = (s.data?.instant?.details?.cloudAreaFraction ?: 0.0) / 100.0,
                precipMm = s.data?.next1Hours?.details?.precipitationAmount ?: 0.0,
            )
        }
    }

    private companion object {
        const val USER_AGENT = "turbo-expressive/0.1 github.com/SigmundGranaas/turbo"
    }
}

/** One forecast step at one sample point: cloud fraction `0..1` + rain (mm/h). */
internal data class SampleStep(val epochMillis: Long, val coverage: Double, val precipMm: Double)

internal const val SAMPLE_COLS = 3
internal const val SAMPLE_ROWS = 3
internal const val GRID_W = 64
internal const val GRID_H = 42
/** Rain rate (mm/h) that maps to full darkness (precip byte 255). */
internal const val PRECIP_FULL_MM = 4.0

/**
 * Resample the per-point timeseries (row-major `cols*rows`) to `frames`
 * [RadarGridFrame]s of `gridW*gridH`, bilinearly interpolating each cell from
 * the surrounding sample points. Pure — unit-tested without the network.
 */
internal fun buildFrames(
    perPoint: List<List<SampleStep>>,
    cols: Int,
    rows: Int,
    gridW: Int,
    gridH: Int,
    frames: Int,
): List<RadarGridFrame> {
    val n = minOf(frames, perPoint.minOf { it.size })
    return (0 until n).map { f ->
        val cov = ByteArray(gridW * gridH)
        val pr = ByteArray(gridW * gridH)
        for (gy in 0 until gridH) {
            val sv = ((gy + 0.5) / gridH) * (rows - 1)
            for (gx in 0 until gridW) {
                val su = ((gx + 0.5) / gridW) * (cols - 1)
                val (coverage, precip) = bilinear(perPoint, cols, rows, su, sv, f)
                val idx = gy * gridW + gx
                cov[idx] = ((coverage.coerceIn(0.0, 1.0)) * 255.0).toInt().toByte()
                pr[idx] = ((precip / PRECIP_FULL_MM).coerceIn(0.0, 1.0) * 255.0).toInt().toByte()
            }
        }
        RadarGridFrame(gridW, gridH, pr, cov, perPoint[0][f].epochMillis)
    }
}

/** Bilinear (coverage, precip) at sample-grid coord (su, sv) for timestep f. */
private fun bilinear(
    perPoint: List<List<SampleStep>>,
    cols: Int,
    rows: Int,
    su: Double,
    sv: Double,
    f: Int,
): Pair<Double, Double> {
    val c0 = su.toInt().coerceIn(0, cols - 1)
    val r0 = sv.toInt().coerceIn(0, rows - 1)
    val c1 = (c0 + 1).coerceAtMost(cols - 1)
    val r1 = (r0 + 1).coerceAtMost(rows - 1)
    val tx = su - c0
    val ty = sv - r0
    fun cov(c: Int, r: Int) = perPoint[r * cols + c][f].coverage
    fun prc(c: Int, r: Int) = perPoint[r * cols + c][f].precipMm
    val coverage = lerp2(cov(c0, r0), cov(c1, r0), cov(c0, r1), cov(c1, r1), tx, ty)
    val precip = lerp2(prc(c0, r0), prc(c1, r0), prc(c0, r1), prc(c1, r1), tx, ty)
    return coverage to precip
}

private fun lerp(a: Double, b: Double, t: Double) = a + (b - a) * t
private fun frac(i: Int, count: Int) = if (count <= 1) 0.5 else i.toDouble() / (count - 1)
private fun lerp2(v00: Double, v10: Double, v01: Double, v11: Double, tx: Double, ty: Double) =
    lerp(lerp(v00, v10, tx), lerp(v01, v11, tx), ty)

// ── locationforecast (compact) — just the fields the overlay needs ──────────
@Serializable
private data class Lf(val properties: LfProps? = null)

@Serializable
private data class LfProps(val timeseries: List<LfSeries> = emptyList())

@Serializable
private data class LfSeries(val time: String? = null, val data: LfData? = null)

@Serializable
private data class LfData(
    val instant: LfInstant? = null,
    @SerialName("next_1_hours") val next1Hours: LfNext? = null,
)

@Serializable
private data class LfInstant(val details: LfInstantDetails? = null)

@Serializable
private data class LfInstantDetails(
    @SerialName("cloud_area_fraction") val cloudAreaFraction: Double? = null,
)

@Serializable
private data class LfNext(val details: LfNextDetails? = null)

@Serializable
private data class LfNextDetails(
    @SerialName("precipitation_amount") val precipitationAmount: Double? = null,
)
