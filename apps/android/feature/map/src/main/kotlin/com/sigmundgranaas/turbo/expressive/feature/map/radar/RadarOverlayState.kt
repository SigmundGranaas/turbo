package com.sigmundgranaas.turbo.expressive.feature.map.radar

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.WeatherCloudOverlay
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Drives the GPU cloud overlay from a loaded radar sequence and a single
 * scrubbable timeline position.
 *
 * The position is continuous over `0 .. frameCount-1`: its integer part selects
 * the current/next frame pair (uploaded to slots 0/1 only when the pair
 * changes — cheap to scrub), and its fractional part is the crossfade `blend`.
 * Dragging the slider, or playback, just moves the position — forward or back.
 */
class RadarOverlayState(private val overlay: WeatherCloudOverlay) {

    var frames: List<RadarFrameData> by mutableStateOf(emptyList())
        private set

    /** Continuous timeline position in `0 .. frameCount-1`. */
    var position by mutableFloatStateOf(0f)
        private set

    var playing by mutableStateOf(false)
        private set

    private var loadedPairIdx = -1

    val frameCount: Int get() = frames.size
    val ready: Boolean get() = frames.size >= 2

    /** Install a freshly loaded sequence and show frame 0. [bounds] is the
     *  lat/lng box the frames cover; passing it world-locks the overlay so the
     *  clouds pan and zoom with the map (null keeps the screen-locked field). */
    fun loadFrames(list: List<RadarFrameData>, bounds: GeoBounds? = null) {
        frames = list
        loadedPairIdx = -1
        if (list.isNotEmpty()) {
            overlay.enableClouds(list.first().gridW, list.first().gridH)
            bounds?.let { overlay.setCloudGeoBounds(it.west, it.south, it.east, it.north) }
            overlay.setCloudsVisible(true)
            seek(0f)
        }
    }

    /** Move the timeline; [p] is clamped into range. Uploads the frame pair if
     *  it changed and sets the crossfade + drift clock. */
    fun seek(p: Float) {
        if (frames.size < 2) return
        val maxP = (frames.size - 1).toFloat()
        val clamped = p.coerceIn(0f, maxP)
        position = clamped
        val idx = clamped.toInt().coerceIn(0, frames.size - 2)
        val blend = clamped - idx
        if (idx != loadedPairIdx) {
            val a = frames[idx]
            val b = frames[idx + 1]
            overlay.ingestRadarFrame(0, a.gridW, a.gridH, a.precip, a.coverage)
            overlay.ingestRadarFrame(1, b.gridW, b.gridH, b.precip, b.coverage)
            loadedPairIdx = idx
        }
        // Drift clock tied to the timeline so clouds move as you scrub/play.
        overlay.setCloudTime(clamped * SECONDS_PER_FRAME, blend)
    }

    fun togglePlay() { playing = !playing }

    fun setVisible(visible: Boolean) = overlay.setCloudsVisible(visible)

    /** Advance the timeline by [dtSeconds] of wall-clock while playing, looping
     *  at the end. Call from the frame loop. */
    fun advance(dtSeconds: Float) {
        if (!playing || frames.size < 2) return
        val maxP = (frames.size - 1).toFloat()
        var p = position + dtSeconds / SECONDS_PER_FRAME
        if (p >= maxP) p = 0f
        seek(p)
    }

    /** `HH:mm` of the frame nearest the current position. */
    fun currentLabel(): String {
        if (frames.isEmpty()) return ""
        val i = position.toInt().coerceIn(0, frames.size - 1)
        return TIME_FMT.format(Date(frames[i].epochMillis))
    }

    companion object {
        /** Seconds of drift per radar frame interval — sets playback speed. */
        const val SECONDS_PER_FRAME = 1.2f
        private val TIME_FMT = SimpleDateFormat("HH:mm", Locale.getDefault())
    }
}
