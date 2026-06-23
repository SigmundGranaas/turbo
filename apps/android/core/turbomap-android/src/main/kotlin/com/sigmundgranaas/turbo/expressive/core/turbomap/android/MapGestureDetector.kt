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
 * **2D:** two-finger gestures apply pan + zoom simultaneously — the centroid translation
 * pans and the spread ratio zooms, both every frame. 2D never rotates or tilts.
 *
 * **3D:** the two-finger DRAG orbits the camera (never pans — panning is one-finger): the
 * centroid's horizontal drag rotates the bearing and its vertical drag tilts the pitch. The
 * finger TWIST does nothing. The spread still pinch-zooms about the centroid.
 */
/** Fallback px/s fling gate for the pure helpers / tests; the detector passes a density-scaled one. */
internal const val MIN_FLING_VELOCITY = 220f

/** Accumulated finger-twist (degrees) before 3D bearing rotation engages — a dead-zone so a
 *  pure pinch/tilt doesn't wobble the bearing. Once engaged, per-frame deltas apply continuously. */
internal const val ROTATE_GATE_DEG = 7f

/** Pitch (tilt) degrees per pixel of two-finger VERTICAL drag in 3D. ~0.3°/px → a ~270 px drag
 *  sweeps the full 0–80° tilt; the engine clamps to its pitch range. Tunable on device. */
internal const val PITCH_PER_PX = 0.3f

/** Bearing (rotate) degrees per pixel of two-finger HORIZONTAL drag in 3D. ~0.3°/px → a ~300 px
 *  drag spins ~90°. Tunable on device (flip the sign if rotation feels reversed). */
internal const val BEARING_PER_PX = 0.3f

/**
 * Signed shortest-arc change between two finger-pair angles (radians in),
 * returned in DEGREES and wrapped to [-180, 180] so a twist across the ±π seam
 * doesn't spike. Pure → unit-tested.
 */
internal fun twistDeltaDeg(prevAngleRad: Float, angleRad: Float): Float {
    var d = Math.toDegrees((angleRad - prevAngleRad).toDouble()).toFloat()
    while (d > 180f) d -= 360f
    while (d < -180f) d += 360f
    return d
}

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

/**
 * Which gesture grammar [detectMapGestures] applies. One finger pans in both modes.
 * Two fingers pan + zoom together in both modes; in **3D** they also rotate the
 * bearing via finger-twist (2D never rotates). There is no tilt gesture.
 * See docs/architecture/2026-06-2d-3d-map-mode-gestures.md.
 */
internal enum class MapGestureMode { TwoD, ThreeD }

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + (3D) twist-rotate + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a touch
 *   stops the map exactly where it is);
 * - **one finger** → [onTransform] pan deltas, ignored until the move passes
 *   [MOVE_SLOP_DP] so a shaky finger doesn't nudge or drift the map (both 2D and 3D);
 * - **two fingers** → in **2D**, pan + zoom together via [onTransform] (centroid translation
 *   pans, spread ratio zooms). In **3D** the DRAG orbits via [onOrbit] — horizontal rotates the
 *   bearing, vertical tilts the pitch; twist does nothing; the spread still pinch-zooms via
 *   [onTransform] with zero pan; two fingers never pan in 3D;
 * - **release** → a pan [onFling] (one-finger velocity) or a [onZoomFling] (two-finger zoom
 *   momentum) — never both; anything else rests.
 *
 * The velocity baseline is reset whenever the pointer count changes, so adding or
 * lifting a finger doesn't inject a spurious centroid jump.
 */
internal suspend fun PointerInputScope.detectMapGestures(
    onDown: () -> Unit,
    onTransform: (panX: Float, panY: Float, zoom: Float, focusX: Float, focusY: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
    onZoomFling: (zoomVelocity: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
    // 3D mode (default off). `mode` is sampled ONCE at gesture start so a toggle mid-gesture
    // doesn't change the grammar underfoot. In 3D two fingers tilt+rotate instead of panning.
    mode: () -> MapGestureMode = { MapGestureMode.TwoD },
    // Two-finger orbit about the centroid (3D only): `dBearingDeg` from the centroid's
    // horizontal drag, `dPitchDeg` from its vertical drag. Both can fire in the same frame.
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
        var wasMulti = false
        var wasPinch = false
        // Single-finger drag only "starts working" once it clears the slop, so a
        // stationary-but-shaky touch neither moves the map nor flings on release.
        var dragStarted = false

        while (true) {
            val event = awaitPointerEvent()
            val pressed = event.changes.filter { it.pressed }
            if (pressed.isEmpty()) break
            val count = pressed.size

            val centroid = pressed.fold(Offset.Zero) { acc, c -> acc + c.position } / count.toFloat()
            val spread = if (count >= 2) {
                pressed.fold(0f) { acc, c -> acc + (c.position - centroid).getDistance() } / count
            } else {
                0f
            }
            val t = pressed.first().uptimeMillis

            if (count != prevCount) {
                // Pointer count changed: re-baseline (a lifted/added finger would otherwise
                // jump the centroid) and restart velocity tracking.
                prevCount = count
                tracker.resetTracking()
                zoomTracker.reset()
                if (count >= 2) {
                    wasMulti = true
                    wasPinch = true
                    dragStarted = true // two fingers are always deliberate
                }
            } else if (count == 1) {
                // One finger pans — in BOTH 2D and 3D — gated by the slop so jitter is ignored.
                if (!dragStarted && (centroid - downPos).getDistance() > moveSlopPx) dragStarted = true
                if (dragStarted) {
                    val pan = centroid - prevCentroid
                    if (pan != Offset.Zero) {
                        onTransform(pan.x, pan.y, 1f, centroid.x, centroid.y)
                        event.changes.forEach { if (it.positionChanged()) it.consume() }
                    }
                }
            } else {
                val pan = centroid - prevCentroid
                val ratio = if (prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (ratio != 1f) zoomTracker.addRatio(t, ratio)

                if (gestureMode == MapGestureMode.ThreeD) {
                    // 3D: the two-finger DRAG orbits the camera — horizontal drag rotates the
                    // bearing, vertical drag tilts the pitch. Finger TWIST does nothing. Two
                    // fingers never pan (panning is one-finger); the spread still pinch-zooms
                    // about the centroid.
                    if (ratio != 1f) onTransform(0f, 0f, ratio, centroid.x, centroid.y)
                    val dBearing = pan.x * BEARING_PER_PX
                    val dPitch = -pan.y * PITCH_PER_PX
                    if (dBearing != 0f || dPitch != 0f) onOrbit(dBearing, dPitch, centroid.x, centroid.y)
                } else {
                    // 2D: pan + zoom together (focus = centroid). 2D never rotates or tilts.
                    onTransform(pan.x, pan.y, ratio, centroid.x, centroid.y)
                }

                event.changes.forEach { if (it.positionChanged()) it.consume() }
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
        }

        // Two-finger zoom carries momentum; pan-from-pinch rests (no sideways drift after a
        // pinch). A one-finger flick carries pan momentum; a sub-slop touch rests.
        if (wasMulti) {
            val zv = zoomTracker.velocity()
            if (shouldZoomFling(zv)) {
                onZoomFling(zv, prevCentroid.x, prevCentroid.y)
            } else {
                onFling(0f, 0f)
            }
        } else if (!dragStarted) {
            onFling(0f, 0f)
        } else {
            val v = tracker.calculateVelocity()
            val (fx, fy) = flingVelocity(v.x, v.y, wasPinch = wasPinch, minVelocity = minFlingPx)
            onFling(fx, fy)
        }
    }
}

/**
 * Tap + long-press that survives a shaky finger, but is strictly SINGLE-FINGER and
 * fires the long-press on RELEASE. Unlike the platform `detectTapGestures`, a touch
 * may wobble up to [MOVE_SLOP_DP] without cancelling (so a long-press still fires on
 * a train). The "selected point" long-press must never appear during a two-finger
 * gesture and only when one finger has been held still and then lifted, so:
 *
 * - a SECOND finger landing at any point cancels the long-press entirely (it's a
 *   pan/zoom/rotate, not a point selection);
 * - holding one finger still (within slop) past the long-press timeout merely *arms*
 *   the long-press — [onLongPress] fires when that finger is **released**, not while
 *   it's down;
 * - a quick release within slop before the timeout fires [onTap];
 * - moving beyond slop is a pan and does neither (the map detector takes over).
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
        var multiFinger = false
        // Phase 1: hold detection. Arm only if ONE finger stays within slop past the
        // timeout. A second finger or a wander past slop bails out early.
        val armed = try {
            withTimeout(longPressTimeout) {
                while (true) {
                    val event = awaitPointerEvent()
                    if (event.changes.count { it.pressed } > 1) {
                        multiFinger = true
                        return@withTimeout false // two fingers → never a point selection
                    }
                    val change = event.changes.firstOrNull { it.id == down.id }
                    if (change == null || !change.pressed) return@withTimeout false // lifted → tap candidate
                    if ((change.position - downPos).getDistance() > slop) {
                        movedTooFar = true
                        return@withTimeout false // a real drag — not a tap or long-press
                    }
                }
                @Suppress("UNREACHABLE_CODE")
                false
            }
        } catch (_: PointerEventTimeoutCancellationException) {
            true // held still, one finger, past the timeout → armed; fire on release
        }
        when {
            armed -> {
                // Phase 2: wait for the SINGLE finger to lift, firing on release. A second
                // finger landing, or wandering past slop, cancels (no menu).
                var cancelled = false
                while (true) {
                    val event = awaitPointerEvent()
                    if (event.changes.count { it.pressed } > 1) cancelled = true
                    val change = event.changes.firstOrNull { it.id == down.id }
                    if (change != null && change.pressed &&
                        (change.position - downPos).getDistance() > slop
                    ) {
                        cancelled = true
                    }
                    if (change == null || !change.pressed) {
                        if (!cancelled) onLongPress(downPos) // released cleanly → select the point
                        break
                    }
                }
            }
            multiFinger -> Unit // two fingers — never a tap/long-press
            movedTooFar -> Unit // became a pan; the map detector owns it
            else -> onTap(downPos) // lifted within slop before the timeout
        }
    }
}
