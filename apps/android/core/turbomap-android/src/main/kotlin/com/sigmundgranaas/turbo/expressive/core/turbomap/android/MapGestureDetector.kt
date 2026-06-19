package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventTimeoutCancellationException
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.hypot
import kotlin.math.ln

/**
 * Minimum release speed (dp/s) before a single-finger drag throws the map. A
 * deliberate flick is well over 1000 dp/s; a slow lift-off or the velocity spike
 * from a shaky finger (e.g. on a train) sits far below this, so it just rests
 * instead of drifting. Expressed in **dp** so the feel is the same on every
 * density; [PointerInputScope] converts it to px/s before gating.
 */
internal const val MIN_FLING_VELOCITY_DP = 160f

/**
 * How far a finger may wander (dp) and still count as a *stationary* touch rather
 * than a pan — the gate for when map physics "starts working". Below this, a
 * touch neither pans the map nor cancels a long-press, so train vibration doesn't
 * nudge the map or steal a long-press; above it, the drag engages. Deliberately
 * larger than the platform touch-slop (~8 dp) because the complaint is jitter.
 */
internal const val MOVE_SLOP_DP = 18f

/**
 * Cumulative twist (degrees) before a two-finger rotate engages. Keeps a pure
 * pinch-zoom (or a shaky two-finger hold) from spinning the map by accident; once
 * the user clearly twists past this, rotation tracks the fingers 1:1.
 */
internal const val MIN_ROTATE_DEG = 7f

/** Fallback px/s fling gate for the pure helpers / tests; the detector passes a density-scaled one. */
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
 * to throw the map sideways afterward ("zoom then drift"). [minVelocity] is the
 * px/s gate (the detector supplies a density-scaled one). Pure → unit-tested.
 */
internal fun flingVelocity(
    vx: Float,
    vy: Float,
    wasPinch: Boolean,
    minVelocity: Float = MIN_FLING_VELOCITY,
): Pair<Float, Float> =
    if (wasPinch || hypot(vx, vy) < minVelocity) 0f to 0f else vx to vy

/** True if a release velocity (px/s) is fast enough to throw the map. */
internal fun shouldFling(vx: Float, vy: Float, minVelocity: Float = MIN_FLING_VELOCITY): Boolean =
    hypot(vx, vy) >= minVelocity

/** Screen angle (degrees) of the vector a→b. y is screen-down, so clockwise is increasing. */
internal fun twoFingerAngleDeg(a: Offset, b: Offset): Float =
    Math.toDegrees(atan2((b.y - a.y).toDouble(), (b.x - a.x).toDouble())).toFloat()

/** Shortest signed difference between two angles (degrees), wrapped to (-180, 180]. */
internal fun wrapDeltaDeg(delta: Float): Float {
    var d = delta % 360f
    if (d > 180f) d -= 360f
    if (d < -180f) d += 360f
    return d
}

/**
 * Which gesture grammar [detectMapGestures] applies. 2D is the legacy model
 * (1-finger pan, pinch zoom + rotate). In 3D a 1-finger drag *orbits* about a
 * pinned focus (horizontal → bearing, vertical → pitch) and two fingers pan + zoom.
 * See docs/architecture/2026-06-2d-3d-map-mode-gestures.md.
 */
internal enum class MapGestureMode { TwoD, ThreeD }

/** 3D orbit sensitivity, screen px → degrees. Horizontal drag spins the bearing. */
internal const val ORBIT_BEARING_DEG_PER_PX = 0.30f

/** 3D orbit sensitivity, screen px → degrees. Up-drag tilts toward the horizon. */
internal const val ORBIT_PITCH_DEG_PER_PX = 0.25f

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + two-finger rotate
 * + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a
 *   touch stops the map exactly where it is);
 * - **single-finger move** is ignored until it passes [MOVE_SLOP_DP] (so a shaky
 *   finger doesn't nudge or drift the map), then → [onTransform] pan deltas;
 * - **two-finger move** → [onTransform] (centroid pan + pinch zoom) applied live,
 *   plus [onRotate] once the twist clears [MIN_ROTATE_DEG];
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
    onTransform: (panX: Float, panY: Float, zoom: Float, focusX: Float, focusY: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
    onZoomFling: (zoomVelocity: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
    onRotate: (dBearingDeg: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
    // 3D mode (default off → identical 2D behaviour). `mode` is sampled ONCE at
    // gesture start so a toggle mid-drag doesn't change the grammar underfoot.
    mode: () -> MapGestureMode = { MapGestureMode.TwoD },
    // The pinned orbit pivot in screen px; null → screen centre.
    orbitFocus: () -> Offset? = { null },
    onOrbit: (dBearingDeg: Float, dPitchDeg: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _, _ -> },
) {
    val moveSlopPx = MOVE_SLOP_DP.dp.toPx()
    val minFlingPx = MIN_FLING_VELOCITY_DP.dp.toPx()
    awaitEachGesture {
        val tracker = VelocityTracker()
        val zoomTracker = ZoomVelocityTracker()
        val first = awaitFirstDown(requireUnconsumed = false)
        onDown()
        val gestureMode = mode()
        tracker.addPosition(first.uptimeMillis, first.position)
        val downPos = first.position
        var prevCount = 1
        var prevCentroid = first.position
        var prevSpread = 0f
        var prevAngle = 0f
        var rotateAccum = 0f
        var rotating = false
        var wasPinch = false
        var wasOrbit = false
        // Single-finger drag only "starts working" once it clears the slop, so a
        // stationary-but-shaky touch neither moves the map nor flings on release.
        var dragStarted = false

        while (true) {
            val event = awaitPointerEvent()
            val pressed = event.changes.filter { it.pressed }
            if (pressed.isEmpty()) break
            if (pressed.size >= 2) {
                wasPinch = true
                dragStarted = true // two fingers are always deliberate
            }

            val centroid = pressed.fold(Offset.Zero) { acc, c -> acc + c.position } / pressed.size.toFloat()
            val spread = if (pressed.size >= 2) {
                pressed.fold(0f) { acc, c -> acc + (c.position - centroid).getDistance() } / pressed.size
            } else {
                0f
            }
            val t = pressed.first().uptimeMillis

            if (pressed.size != prevCount) {
                // Pointer count changed: re-baseline (a lifted/added finger would
                // otherwise jump the centroid) and restart velocity + rotate tracking.
                prevCount = pressed.size
                tracker.resetTracking()
                zoomTracker.reset()
                rotating = false
                rotateAccum = 0f
                if (pressed.size >= 2) {
                    val (a, b) = twoSortedByY(pressed[0].position, pressed[1].position, pressed)
                    prevAngle = twoFingerAngleDeg(a, b)
                }
            } else if (gestureMode == MapGestureMode.ThreeD && pressed.size == 1) {
                // 3D 1-finger → orbit about the pinned focus, once past the slop so
                // a shaky finger doesn't spin/tilt the map. Horizontal spins the
                // bearing, vertical tilts the pitch.
                if (!dragStarted && (centroid - downPos).getDistance() > moveSlopPx) dragStarted = true
                if (dragStarted) {
                    val pan = centroid - prevCentroid
                    if (pan != Offset.Zero) {
                        wasOrbit = true
                        val f = orbitFocus()
                        val onScreen = f != null &&
                            f.x in 0f..size.width.toFloat() && f.y in 0f..size.height.toFloat()
                        val focus = if (onScreen) f!! else Offset(size.width / 2f, size.height / 2f)
                        onOrbit(
                            pan.x * ORBIT_BEARING_DEG_PER_PX,
                            -pan.y * ORBIT_PITCH_DEG_PER_PX,
                            focus.x,
                            focus.y,
                        )
                        event.changes.forEach { if (it.positionChanged()) it.consume() }
                    }
                }
            } else if (pressed.size == 1) {
                // 2D single-finger pan, gated by the slop so jitter is ignored.
                if (!dragStarted && (centroid - downPos).getDistance() > moveSlopPx) dragStarted = true
                if (dragStarted) {
                    val pan = centroid - prevCentroid
                    if (pan != Offset.Zero) {
                        onTransform(pan.x, pan.y, 1f, centroid.x, centroid.y)
                        event.changes.forEach { if (it.positionChanged()) it.consume() }
                    }
                }
            } else {
                // Two fingers: pan + pinch-zoom (live) and a gated two-finger rotate.
                val pan = centroid - prevCentroid
                val zoom = if (prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (zoom != 1f) zoomTracker.addRatio(t, zoom)
                if (pan != Offset.Zero || zoom != 1f) onTransform(pan.x, pan.y, zoom, centroid.x, centroid.y)

                val (a, b) = twoSortedByY(pressed[0].position, pressed[1].position, pressed)
                val angle = twoFingerAngleDeg(a, b)
                val dAngle = wrapDeltaDeg(angle - prevAngle)
                if (!rotating) {
                    rotateAccum += dAngle
                    if (abs(rotateAccum) >= MIN_ROTATE_DEG) rotating = true
                }
                if (rotating && dAngle != 0f) onRotate(dAngle, centroid.x, centroid.y)
                prevAngle = angle
                event.changes.forEach { if (it.positionChanged()) it.consume() }
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
        }

        // A pinch carries zoom momentum (locked to the zoom axis); a 2D
        // single-finger flick carries pan momentum; a 3D orbit just rests (no
        // bearing/pitch fling for now). A sub-slop touch never flings.
        if (wasPinch) {
            val zv = zoomTracker.velocity()
            if (shouldZoomFling(zv)) onZoomFling(zv, prevCentroid.x, prevCentroid.y) else onFling(0f, 0f)
        } else if (wasOrbit || !dragStarted) {
            onFling(0f, 0f)
        } else {
            val v = tracker.calculateVelocity()
            val (fx, fy) = flingVelocity(v.x, v.y, wasPinch = false, minVelocity = minFlingPx)
            onFling(fx, fy)
        }
    }
}

/**
 * Order the two active pointers by screen-Y so the finger→finger vector (and thus
 * its angle) is stable frame-to-frame regardless of the event's pointer order.
 * Falls back to the passed positions when fewer than two are available.
 */
private fun twoSortedByY(
    p0: Offset,
    p1: Offset,
    pressed: List<androidx.compose.ui.input.pointer.PointerInputChange>,
): Pair<Offset, Offset> {
    if (pressed.size < 2) return p0 to p1
    val sorted = pressed.map { it.position }.sortedWith(compareBy({ it.y }, { it.x }))
    return sorted[0] to sorted[1]
}

/**
 * Tap + long-press that survives a shaky finger. Unlike the platform
 * `detectTapGestures`, a touch is allowed to wobble up to [MOVE_SLOP_DP] without
 * cancelling — so a long-press still fires on a train (the user's finger can't be
 * perfectly still). A touch held past the long-press timeout within that slop
 * fires [onLongPress]; a quick release within it fires [onTap]; moving beyond it
 * is a pan and does neither (the map gesture detector takes over).
 */
internal suspend fun PointerInputScope.detectTapAndLongPress(
    onTap: (Offset) -> Unit,
    onLongPress: (Offset) -> Unit,
) {
    val slop = MOVE_SLOP_DP.dp.toPx()
    val longPressTimeout = viewConfiguration.longPressTimeoutMillis
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        val downPos = down.position
        var movedTooFar = false
        val longPressed = try {
            withTimeout(longPressTimeout) {
                while (true) {
                    val event = awaitPointerEvent()
                    val change = event.changes.firstOrNull { it.id == down.id }
                    if (change == null || !change.pressed) return@withTimeout false // lifted → a tap candidate
                    if ((change.position - downPos).getDistance() > slop) {
                        movedTooFar = true
                        return@withTimeout false // a real drag — not a tap or long-press
                    }
                }
                @Suppress("UNREACHABLE_CODE")
                false
            }
        } catch (_: PointerEventTimeoutCancellationException) {
            true // held within slop past the timeout → long-press
        }
        when {
            longPressed -> {
                onLongPress(downPos)
                // Drain the rest of the gesture so the eventual lift isn't read as a tap.
                do {
                    val event = awaitPointerEvent()
                    val change = event.changes.firstOrNull { it.id == down.id }
                    if (change == null || !change.pressed) break
                } while (true)
            }
            movedTooFar -> Unit // became a pan; the map detector owns it
            else -> onTap(downPos) // lifted within slop before the timeout
        }
    }
}
