package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.input.pointer.util.VelocityTracker
import kotlin.math.hypot

/** Don't fling on a slow release / tap — only a real flick gets momentum. */
internal const val MIN_FLING_VELOCITY = 120f

/** True if a release velocity (px/s) is fast enough to throw the map. Pure → unit-tested. */
internal fun shouldFling(vx: Float, vy: Float): Boolean = hypot(vx, vy) >= MIN_FLING_VELOCITY

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a
 *   touch stops the map exactly where it is);
 * - **move** → [onTransform] with the centroid pan delta + pinch zoom factor
 *   (applied live), while a [VelocityTracker] follows the centroid;
 * - **release** → [onFling] with the centroid velocity (px/s) for the momentum
 *   throw — `(0,0)` when the flick was too slow, so it just rests.
 *
 * The velocity baseline is reset whenever the pointer count changes, so adding
 * or lifting a finger during a pinch doesn't inject a spurious centroid jump.
 */
internal suspend fun PointerInputScope.detectMapGestures(
    onDown: () -> Unit,
    onTransform: (panX: Float, panY: Float, zoom: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
) {
    awaitEachGesture {
        val tracker = VelocityTracker()
        val first = awaitFirstDown(requireUnconsumed = false)
        onDown()
        tracker.addPosition(first.uptimeMillis, first.position)
        var prevCount = 1
        var prevCentroid = first.position
        var prevSpread = 0f

        while (true) {
            val event = awaitPointerEvent()
            val pressed = event.changes.filter { it.pressed }
            if (pressed.isEmpty()) break

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
            } else {
                val pan = centroid - prevCentroid
                val zoom = if (pressed.size >= 2 && prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (pan != Offset.Zero || zoom != 1f) onTransform(pan.x, pan.y, zoom)
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
            event.changes.forEach { if (it.positionChanged()) it.consume() }
        }

        val v = tracker.calculateVelocity()
        if (shouldFling(v.x, v.y)) onFling(v.x, v.y) else onFling(0f, 0f)
    }
}
