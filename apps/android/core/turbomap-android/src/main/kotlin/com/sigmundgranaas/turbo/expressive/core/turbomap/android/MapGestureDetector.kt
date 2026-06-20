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
 * Movement gates for the two-finger gesture. Each of zoom / rotate / tilt only
 * *engages* once its own signal crosses a deliberate threshold, so a simple pinch
 * doesn't also rotate or tilt, and a twist doesn't also zoom — the map stops
 * "moving all over the place" during a plain pinch or pan. Once an axis engages it
 * tracks the fingers; the small pre-gate motion is discarded as a dead-zone.
 */
/** Cumulative twist (deg) before rotation engages — a clearly deliberate turn. */
internal const val ROTATE_GATE_DEG = 11f

/** While a zoom is already engaged, rotation needs a much bigger twist to kick in
 *  ("…unless you reaaaaaally push it") so a pinch-zoom doesn't spin the map. */
internal const val ROTATE_GATE_WHILE_ZOOMING_DEG = 28f

/** Cumulative zoom (levels) before the pinch-zoom engages — a tiny dead-zone so a
 *  pure twist or tilt doesn't nudge the zoom. */
internal const val ZOOM_GATE_LEVELS = 0.05f

/** Cumulative parallel-vertical travel (dp) before a two-finger tilt engages (3D only). */
internal const val TILT_GATE_DP = 26f

/** Fallback px/s fling gate for the pure helpers / tests; the detector passes a density-scaled one. */
internal const val MIN_FLING_VELOCITY = 220f

/** The rotate engagement gate (deg): far stiffer while a zoom is in progress. */
internal fun rotateGateDeg(zooming: Boolean): Float =
    if (zooming) ROTATE_GATE_WHILE_ZOOMING_DEG else ROTATE_GATE_DEG

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

/**
 * The two-finger angle with a STABLE pointer order (sorted by screen-Y then X), so the
 * finger→finger vector — and thus the twist delta — doesn't flip when the event reports
 * the two pointers in a different order frame-to-frame.
 */
internal fun twoFingerAngleDeg(
    pressed: List<androidx.compose.ui.input.pointer.PointerInputChange>,
): Float {
    if (pressed.size < 2) return 0f
    val sorted = pressed.map { it.position }.sortedWith(compareBy({ it.y }, { it.x }))
    return twoFingerAngleDeg(sorted[0], sorted[1])
}

/** Shortest signed difference between two angles (degrees), wrapped to (-180, 180]. */
internal fun wrapDeltaDeg(delta: Float): Float {
    var d = delta % 360f
    if (d > 180f) d -= 360f
    if (d < -180f) d += 360f
    return d
}

/**
 * Which gesture grammar [detectMapGestures] applies. In BOTH modes one finger pans;
 * two fingers zoom (pinch) + rotate (twist) about the centroid. 3D additionally lets
 * a two-finger parallel vertical drag *tilt* the pitch; 2D pins the pitch flat.
 * See docs/architecture/2026-06-2d-3d-map-mode-gestures.md.
 */
internal enum class MapGestureMode { TwoD, ThreeD }

/** Two-finger tilt sensitivity, screen px → degrees of pitch. Up-drag tilts toward the horizon. */
internal const val ORBIT_PITCH_DEG_PER_PX = 0.25f

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + two-finger rotate +
 * (3D) tilt + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a touch
 *   stops the map exactly where it is);
 * - **one finger** → [onTransform] pan deltas, ignored until the move passes
 *   [MOVE_SLOP_DP] so a shaky finger doesn't nudge or drift the map (both 2D and 3D);
 * - **two fingers** → zoom about the centroid via [onTransform] (pan stays 0 — one
 *   finger pans), plus [onOrbit] rotate (twist) and, in 3D, tilt (parallel vertical
 *   drag). Each of zoom / rotate / tilt only engages once its own movement gate is
 *   crossed, so a plain pinch doesn't rotate/tilt and a twist doesn't zoom;
 * - **release** → a pan [onFling] (one-finger velocity) or a [onZoomFling] (pinch
 *   rate) — never both; a slow release rests.
 *
 * The velocity baseline is reset whenever the pointer count changes, so adding or
 * lifting a finger doesn't inject a spurious centroid jump.
 */
internal suspend fun PointerInputScope.detectMapGestures(
    onDown: () -> Unit,
    onTransform: (panX: Float, panY: Float, zoom: Float, focusX: Float, focusY: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
    onZoomFling: (zoomVelocity: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
    // 3D mode (default off). `mode` is sampled ONCE at gesture start so a toggle
    // mid-drag doesn't change the grammar underfoot. The only difference: 3D allows
    // a two-finger tilt; 2D keeps the pitch flat.
    mode: () -> MapGestureMode = { MapGestureMode.TwoD },
    // Two-finger rotate (bearing) + tilt (pitch, 3D only) about the gesture centroid.
    onOrbit: (dBearingDeg: Float, dPitchDeg: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _, _ -> },
) {
    val moveSlopPx = MOVE_SLOP_DP.dp.toPx()
    val minFlingPx = MIN_FLING_VELOCITY_DP.dp.toPx()
    val tiltGatePx = TILT_GATE_DP.dp.toPx()
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
        // Per-axis engagement gates for the two-finger gesture (movement gates).
        var rotateAccum = 0f; var rotating = false
        var zoomAccumLevels = 0f; var zooming = false
        var tiltAccumPx = 0f; var tilting = false
        var wasMulti = false
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
                // jump the centroid) and restart velocity + the two-finger gates.
                prevCount = count
                tracker.resetTracking()
                zoomTracker.reset()
                rotating = false; rotateAccum = 0f
                zooming = false; zoomAccumLevels = 0f
                tilting = false; tiltAccumPx = 0f
                if (count >= 2) {
                    wasMulti = true
                    dragStarted = true // two fingers are always deliberate
                    prevAngle = twoFingerAngleDeg(pressed)
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
                // Two fingers: zoom about the centroid + rotate + (3D) tilt. NO pan — one
                // finger pans — so a pinch can't slide the map around. Each axis is gated.
                var applied = false

                // ZOOM — pinch. A tiny dead-zone so a pure twist/tilt doesn't nudge zoom.
                val ratio = if (prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (ratio != 1f) {
                    if (!zooming) {
                        zoomAccumLevels += abs(ln(ratio.toDouble()).toFloat() / LN2)
                        if (zoomAccumLevels >= ZOOM_GATE_LEVELS) zooming = true
                    }
                    if (zooming) {
                        zoomTracker.addRatio(t, ratio)
                        onTransform(0f, 0f, ratio, centroid.x, centroid.y)
                        applied = true
                    }
                }

                // ROTATE — twist. Stiffer gate while zooming so a pinch doesn't spin the map.
                val angle = twoFingerAngleDeg(pressed)
                val dAngle = wrapDeltaDeg(angle - prevAngle)
                if (!rotating) {
                    rotateAccum += dAngle
                    if (abs(rotateAccum) >= rotateGateDeg(zooming)) rotating = true
                }
                prevAngle = angle

                // TILT — parallel vertical drag (3D only).
                val dy = centroid.y - prevCentroid.y
                var dPitch = 0f
                if (gestureMode == MapGestureMode.ThreeD) {
                    if (!tilting) {
                        tiltAccumPx += abs(dy)
                        if (tiltAccumPx >= tiltGatePx) tilting = true
                    }
                    if (tilting) dPitch = -dy * ORBIT_PITCH_DEG_PER_PX
                }

                // Apply rotate + tilt about the centroid. Bearing sign is flipped so the map
                // turns WITH the fingers (a clockwise twist rotates the map clockwise).
                val dBearing = if (rotating) -dAngle else 0f
                if (dBearing != 0f || dPitch != 0f) {
                    onOrbit(dBearing, dPitch, centroid.x, centroid.y)
                    applied = true
                }

                if (applied) event.changes.forEach { if (it.positionChanged()) it.consume() }
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
        }

        // A two-finger gesture carries zoom momentum (locked to the zoom axis, no sideways
        // drift); a one-finger flick carries pan momentum; a sub-slop touch rests.
        if (wasMulti) {
            val zv = zoomTracker.velocity()
            if (shouldZoomFling(zv)) onZoomFling(zv, prevCentroid.x, prevCentroid.y) else onFling(0f, 0f)
        } else if (!dragStarted) {
            onFling(0f, 0f)
        } else {
            val v = tracker.calculateVelocity()
            val (fx, fy) = flingVelocity(v.x, v.y, wasPinch = false, minVelocity = minFlingPx)
            onFling(fx, fy)
        }
    }
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
