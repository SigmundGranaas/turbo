package com.sigmundgranaas.turbo.expressive.feature.map.radar

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import kotlin.math.exp
import kotlin.math.sqrt

/**
 * Produces a short, ordered sequence of [RadarFrameData] for a viewport — the
 * recent radar past through the near-future nowcast — which the overlay
 * crossfades and a time slider scrubs.
 *
 * Two implementations:
 * - [SyntheticRadarDataSource] — a self-contained moving storm, so the feature
 *   demos with no network (and is what the offscreen Rust test renders too).
 * - [MetRadarDataSource] — the live MET Norway path (skeleton; see its docs).
 */
fun interface RadarDataSource {
    /**
     * Frames covering [bounds], in chronological order. [frameCount] is a hint;
     * implementations may return fewer.
     */
    suspend fun load(bounds: GeoBounds, frameCount: Int): List<RadarFrameData>
}

/**
 * A drifting frontal rain band with surrounding fair-weather cloud over
 * otherwise clear sky — the same model the Rust `SyntheticStorm` renders, so
 * the on-device look matches the offscreen verification. Deterministic and
 * offline; ideal for development and screenshots.
 */
class SyntheticRadarDataSource(
    private val gridW: Int = 64,
    private val gridH: Int = 42,
    private val minutesPerFrame: Long = 5,
) : RadarDataSource {

    override suspend fun load(bounds: GeoBounds, frameCount: Int): List<RadarFrameData> {
        val n = frameCount.coerceIn(2, 24)
        val now = System.currentTimeMillis()
        return (0 until n).map { fi ->
            val t = fi.toFloat() / (n - 1)
            val precip = ByteArray(gridW * gridH)
            val coverage = ByteArray(gridW * gridH)
            val front = -0.2f + t * 1.3f
            for (y in 0 until gridH) {
                for (x in 0 until gridW) {
                    val nx = (x + 0.5f) / gridW
                    val ny = (y + 0.5f) / gridH
                    val band = gauss(nx + (ny - 0.5f) * 0.3f - front, 0.10f)
                    // A couple of drifting fair-weather masses.
                    var mass = 0f
                    mass += gauss(dist(nx, ny, 0.78f + t * 0.9f, 0.32f), 0.16f) * 0.85f
                    mass += gauss(dist(nx, ny, 0.30f + t * 0.9f, 0.66f), 0.18f) * 0.9f
                    val cov = (band * 0.85f + mass).coerceIn(0f, 1f)
                    val pr = (band * band * 0.7f).coerceIn(0f, 1f)
                    coverage[y * gridW + x] = (cov * 255f).toInt().toByte()
                    precip[y * gridW + x] = (pr * 255f).toInt().toByte()
                }
            }
            RadarFrameData(gridW, gridH, precip, coverage, now + fi * minutesPerFrame * 60_000L)
        }
    }

    private fun gauss(d: Float, sigma: Float): Float = exp(-(d * d) / (2f * sigma * sigma))
    private fun dist(x: Float, y: Float, cx: Float, cy: Float): Float {
        val dx = x - cx
        val dy = y - cy
        return sqrt(dx * dx + dy * dy)
    }
}
