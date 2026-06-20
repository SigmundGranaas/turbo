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
 * The two-finger gesture commits to ONE intent — **zoom** (pinch) or **drag** —
 * decided by whether the fingers' *spread* changed (pinch) or their *centroid* moved
 * (drag), whichever crosses its gate first (see [lockTwoFingerAxis]). There is no
 * finger-twist gesture. A "drag" is a free move: in 3D it orbits the camera
 * (left/right → bearing, up/down → pitch), in 2D it pans. The zoom gate is wide so a
 * plain two-finger drag rarely trips an accidental zoom.
 */
/** Net spread change (zoom levels) before a pinch wins the gesture — deliberately wide
 *  so dragging two fingers to pan/orbit doesn't keep tripping zoom. */
internal const val ZOOM_GATE_LEVELS = 0.16f

/** Centroid travel (dp) before a two-finger drag (pan / orbit) wins the gesture. */
internal const val DRAG_GATE_DP = 14f

/** Two-finger orbit sensitivity (3D), screen px → degrees. Horizontal drag spins the bearing. */
internal const val ORBIT_BEARING_DEG_PER_PX = 0.22f

/** Fallback px/s fling gate for the pure helpers / tests; the detector passes a density-scaled one. */
internal const val MIN_FLING_VELOCITY = 220f

/** The one intent a two-finger gesture commits to. Drag = pan (2D) / orbit (3D). */
internal enum class TwoFingerAxis { Zoom, Drag }

/**
 * Decide the gesture's intent. [zoomN] is the net spread change ÷ its gate; [dragN] is
 * the centroid travel ÷ its gate (1.0 = "just reached the gate"). Returns null until one
 * crosses 1.0 (a dead-zone); then the *more* progressed intent wins and owns the rest of
 * the gesture — zoom XOR drag. Ties favour drag, so an ambiguous gesture pans rather than
 * zooms (the common complaint was accidental zoom while panning).
 */
internal fun lockTwoFingerAxis(zoomN: Float, dragN: Float): TwoFingerAxis? = when {
    zoomN < 1f && dragN < 1f -> null
    zoomN > dragN -> TwoFingerAxis.Zoom
    else -> TwoFingerAxis.Drag
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
 * Which gesture grammar [detectMapGestures] applies. In BOTH modes one finger pans and
 * two fingers either pinch-zoom or drag. A two-finger DRAG orbits the camera in 3D
 * (left/right → bearing, up/down → pitch) and pans in 2D. There is no finger-twist.
 * See docs/architecture/2026-06-2d-3d-map-mode-gestures.md.
 */
internal enum class MapGestureMode { TwoD, ThreeD }

/** Two-finger orbit sensitivity (3D), screen px → degrees of pitch. Up-drag tilts toward the horizon. */
internal const val ORBIT_PITCH_DEG_PER_PX = 0.22f

/**
 * iPhone-style map gestures: one detector for pan + pinch-zoom + two-finger orbit + momentum.
 *
 * - **finger down** → [onDown] (the host catches any in-flight animation, so a touch
 *   stops the map exactly where it is);
 * - **one finger** → [onTransform] pan deltas, ignored until the move passes
 *   [MOVE_SLOP_DP] so a shaky finger doesn't nudge or drift the map (both 2D and 3D);
 * - **two fingers** → ONE intent per gesture (see [lockTwoFingerAxis]): **zoom** (pinch,
 *   spread change) via [onTransform], OR a **drag** (the centroid moves). A drag orbits
 *   in 3D via [onOrbit] — left/right → bearing, up/down → pitch — and pans in 2D via
 *   [onTransform]. There is no finger-twist. The zoom gate is wide, so a two-finger drag
 *   rarely trips an accidental zoom;
 * - **release** → a pan [onFling] (one-finger velocity) or a [onZoomFling] (only if the
 *   gesture locked to zoom) — never both; anything else rests.
 *
 * The velocity baseline is reset whenever the pointer count changes, so adding or
 * lifting a finger doesn't inject a spurious centroid jump.
 */
internal suspend fun PointerInputScope.detectMapGestures(
    onDown: () -> Unit,
    onTransform: (panX: Float, panY: Float, zoom: Float, focusX: Float, focusY: Float) -> Unit,
    onFling: (vx: Float, vy: Float) -> Unit,
    onZoomFling: (zoomVelocity: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _ -> },
    // 3D mode (default off). `mode` is sampled ONCE at gesture start so a toggle mid-drag
    // doesn't change the grammar underfoot. The only difference: a two-finger drag orbits
    // in 3D and pans in 2D.
    mode: () -> MapGestureMode = { MapGestureMode.TwoD },
    // Two-finger orbit (3D): bearing + pitch about the gesture centroid.
    onOrbit: (dBearingDeg: Float, dPitchDeg: Float, focusX: Float, focusY: Float) -> Unit = { _, _, _, _ -> },
) {
    val moveSlopPx = MOVE_SLOP_DP.dp.toPx()
    val minFlingPx = MIN_FLING_VELOCITY_DP.dp.toPx()
    val dragGatePx = DRAG_GATE_DP.dp.toPx()
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
        // Two-finger arbitration: pinch (net spread) vs drag (centroid travel from where the
        // second finger landed); the first to cross its gate locks the gesture.
        var lockedAxis: TwoFingerAxis? = null
        var zoomAccumLevels = 0f
        var startCentroid = first.position
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
                lockedAxis = null
                zoomAccumLevels = 0f
                if (count >= 2) {
                    wasMulti = true
                    dragStarted = true // two fingers are always deliberate
                    startCentroid = centroid // measure the drag from here
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
                // Two fingers commit to ONE intent — pinch-zoom OR drag. Zoom is the SIGNED net
                // spread change; drag is how far the centroid has travelled from where the second
                // finger landed. The wide zoom gate + signed-net spread mean a two-finger drag
                // (which keeps the spread ~constant) doesn't keep tripping an accidental zoom.
                val ratio = if (prevSpread > 0f && spread > 0f) spread / prevSpread else 1f
                if (ratio != 1f) zoomAccumLevels += ln(ratio.toDouble()).toFloat() / LN2
                val dx = centroid.x - prevCentroid.x
                val dy = centroid.y - prevCentroid.y

                if (lockedAxis == null) {
                    lockedAxis = lockTwoFingerAxis(
                        zoomN = abs(zoomAccumLevels) / ZOOM_GATE_LEVELS,
                        dragN = (centroid - startCentroid).getDistance() / dragGatePx,
                    )
                }

                var applied = false
                when (lockedAxis) {
                    TwoFingerAxis.Zoom -> if (ratio != 1f) {
                        zoomTracker.addRatio(t, ratio)
                        onTransform(0f, 0f, ratio, centroid.x, centroid.y)
                        applied = true
                    }
                    // A drag: orbit in 3D (left/right → bearing, up/down → pitch), pan in 2D.
                    TwoFingerAxis.Drag -> if (dx != 0f || dy != 0f) {
                        if (gestureMode == MapGestureMode.ThreeD) {
                            onOrbit(dx * ORBIT_BEARING_DEG_PER_PX, -dy * ORBIT_PITCH_DEG_PER_PX, centroid.x, centroid.y)
                        } else {
                            onTransform(dx, dy, 1f, centroid.x, centroid.y)
                        }
                        applied = true
                    }
                    null -> {} // still in the dead-zone — nothing has won yet
                }

                if (applied) event.changes.forEach { if (it.positionChanged()) it.consume() }
            }
            prevCentroid = centroid
            prevSpread = spread
            tracker.addPosition(t, centroid)
        }

        // Only a zoom gesture carries momentum (locked to the zoom axis, no sideways drift);
        // an orbit rests; a one-finger flick carries pan momentum; a sub-slop touch rests.
        if (wasMulti) {
            val zv = zoomTracker.velocity()
            if (lockedAxis == TwoFingerAxis.Zoom && shouldZoomFling(zv)) {
                onZoomFling(zv, prevCentroid.x, prevCentroid.y)
            } else {
                onFling(0f, 0f)
            }
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
