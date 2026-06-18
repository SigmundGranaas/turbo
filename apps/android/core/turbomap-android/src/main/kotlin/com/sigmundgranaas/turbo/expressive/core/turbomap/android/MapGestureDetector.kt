package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.input.pointer.util.VelocityTracker
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.ln

/** Don't fling on a slow release / tap / tiny drift — only a deliberate flick. */
internal const val MIN_FLING_VELOCITY = 220f

/** ln(2): converts a natural-log scale rate into zoom-levels/second. */
private const val LN2 = 0.6931472f

/** Below this pinch speed (zoom-levels/s) the release just rests — no momentum zoom. */
internal const val MIN_ZOOM_FLING_VELOCITY = 0.7f

/** True if a pinch-release zoom rate (levels/s) is fast enough to coast. */
internal fun shouldZoomFling(zoomVelocity: Float): Boolean =
    abs(zoomVelocity) >= MIN_ZOOM_FLING_VELOCITY

/**
 * Tracks the zoom rate of a pinch so a release can carry zoom momentum. Each
 * frame's spread *ratio* (`spread / prevSpread`) is accumulated as natural-log
 * scale; [velocity] reports the recent slope in **zoom-levels/second** (positive
 * = zooming in) over a short trailing window, so a pinch that decelerates before
 * release reports ~0 (rests) just like the pan [VelocityTracker]. Pure → tested.
 */
internal class ZoomVelocityTracker(private val windowMs: Long = 100L) {
    private val times = ArrayDeque<Long>()
    private val logScale = ArrayDeque<Float>()
    private var cumLogScale = 0f

    /** Fold one frame's spread ratio (must be > 0) in at time [timeMs]. */
    fun addRatio(timeMs: Long, ratio: Float) {
        if (ratio <= 0f) return
        cumLogScale += ln(ratio)
        times.addLast(timeMs)
        logScale.addLast(cumLogScale)
        while (times.size > 2 && timeMs - times.first() > windowMs) {
            times.removeFirst()
            logScale.removeFirst()
        }
    }

    /** Recent zoom rate in zoom-levels/second (positive = zooming in). */
    fun velocity(): Float {
        if (times.size < 2) return 0f
        val dtSec = (times.last() - times.first()) / 1000f
        if (dtSec <= 0f) return 0f
        return (logScale.last() - logScale.first()) / dtSec / LN2
    }

    fun reset() {
        times.clear()
        logScale.clear()
        cumLogScale = 0f
    }
}

/**
 * Decide the release momentum (px/s). Returns `(0,0)` — i.e. no fling, the map
 * just rests — when the flick was too slow (small-motion drift) OR the gesture
 * involved a second finger ([wasPinch]): a pinch is a zoom, and we don't want it
 * to throw the map sideways afterward ("zoom then drift"). Pure → unit-tested.
 */
internal fun flingVelocity(vx: Float, vy: Float, wasPinch: Boolean): Pair<Float, Float> =
    if (wasPinch || hypot(vx, vy) < MIN_FLING_VELOCITY) 0f to 0f else vx to vy

/** True if a release velocity (px/s) is fast enough to throw the map. */
internal fun shouldFling(vx: Float, vy: Float): Boolean = hypot(vx, vy) >= MIN_FLING_VELOCITY

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a
 *   touch stops the map exactly where it is);
 * - **move** → [onTransform] with the centroid pan delta + pinch zoom factor
 *   (applied live), while a [VelocityTracker] follows the centroid;
 * - **release** → either a pan [onFling] (single-finger centroid velocity, px/s)
 *   or a [onZoomFling] (pinch zoom rate, levels/s) — never both. A pinch release
 *   carries only zoom momentum, locked to the zoom axis (no sideways drift); a
 *   slow release rests.
 *
 * The velocity baseline is reset whenever the pointer count changes, so adding
 * or lifting a finger during a pinch doesn't inject a spurious centroid jump.
 */
internal suspend fun PointerInputScope.detectMapGestures(
    onDown: () -> Unit,
    onTransform: (panX: Float, panY: Float, zoom: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
    onZoomFling: (zoomVelocity: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
) {
    awaitEachGesture {
        val tracker = VelocityTracker()
        val zoomTracker = ZoomVelocityTracker()
        val first = awaitFirstDown(requireUnconsumed = false)
        onDown()
        tracker.addPosition(first.uptimeMillis, first.position)
        var prevCount = 1
        var prevCentroid = first.position
        var prevSpread = 0f
        var wasPinch = false

        while (true) {
            val event = awaitPointerEvent()
            val pressed = event.changes.filter { it.pressed }
            if (pressed.isEmpty()) break
            if (pressed.size >= 2) wasPinch = true

            val centroid = pressed.fold(Offset.Zero) { acc, c -> acc + c.position } / pressed.size.toFloat()
            val spread = if (pressed.size >= 2) {
                pressed.fold(0f) { acc, c -> acc + (c.position - centroid).getDistance() } / pressed.size
            } else {
                0f
            }
            val t = pressed.first().uptimeMillis

            if (pressed.size != prevCount) {
                // Pointer count changed: re-baseline (a lifted/added finger would
                // otherwise jump the centroid) and restart velocity tracking.
                prevCount = pressed.size
                tracker.resetTracking()
                zoomTracker.reset()
            } else {
                val pan = centroid - prevCentroid
                val zoom = if (pressed.size >= 2 && prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (zoom != 1f) zoomTracker.addRatio(t, zoom)
                if (pan != Offset.Zero || zoom != 1f) onTransform(pan.x, pan.y, zoom)
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
            event.changes.forEach { if (it.positionChanged()) it.consume() }
        }

        // A pinch carries zoom momentum (locked to the zoom axis) and never a
        // pan fling; a single-finger flick carries pan momentum. Either may rest.
        if (wasPinch) {
            val zv = zoomTracker.velocity()
            if (shouldZoomFling(zv)) onZoomFling(zv, prevCentroid.x, prevCentroid.y) else onFling(0f, 0f)
        } else {
            val v = tracker.calculateVelocity()
            val (fx, fy) = flingVelocity(v.x, v.y, wasPinch = false)
            onFling(fx, fy)
        }
    }
}
