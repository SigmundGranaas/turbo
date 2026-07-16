package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.BitmapFactory
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.TripOrigin
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.components.MarkerPin
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.roundToInt
import kotlin.math.sin

/** A geotagged photo (or photo cluster) rendered on the map as a framed thumbnail. */
data class PhotoPin(val id: String, val lat: Double, val lng: Double, val count: Int, val coverPath: String?)

/**
 * The renderer-agnostic Compose overlay layer: markers, editable route waypoints, and
 * photo pins, **projected through the [MapEngine] seam** ([MapEngine.toScreen] /
 * [MapEngine.fromScreen]) rather than any one renderer's projection. Both the MapLibre
 * host (`TurboMap`) and the wgpu host (`TurbomapMapView`) draw their pins with this, so
 * the on-map UI is pixel-identical regardless of which engine is behind it.
 *
 * [cameraTick] is bumped by the host on every camera change so the offsets reproject in
 * lockstep with the map (markers don't drift during pan/zoom). Place this in the same
 * box as the map surface, filling it.
 */
@Composable
fun MapOverlay(
    engine: MapEngine,
    cameraTick: Int,
    modifier: Modifier = Modifier,
    markers: List<Marker> = emptyList(),
    selectedMarkerId: String? = null,
    markerFallbackColor: Color = Color(0xFF8F4C38),
    /** Live device position, drawn as a [MyPositionPin] that stands ON the terrain (projected
     *  through the engine seam) rather than draped flat. Null = not located / hidden. */
    userLocation: LatLng? = null,
    /** Course over ground in degrees (0 = N); when non-null the pin shows a heading beam whose
     *  on-screen direction is derived via the engine projection (correct under map rotation/tilt). */
    userHeading: Float? = null,
    /** My-position dot colour; null = the default blue. A settings customization. */
    userDotColor: Color? = null,
    onMarkerClick: (Marker) -> Unit = {},
    photoPins: List<PhotoPin> = emptyList(),
    onPhotoPinClick: (PhotoPin) -> Unit = {},
    waypoints: List<LatLng> = emptyList(),
    selectedWaypoint: Int? = null,
    onWaypointTap: (Int) -> Unit = {},
    onWaypointLongPress: (Int) -> Unit = {},
    onWaypointMoved: (Int, LatLng) -> Unit = { _, _ -> },
    /** A waypoint drag began ([Int] = index): the host selects it + suppresses route re-solve. */
    onWaypointDragStart: (Int) -> Unit = {},
    /** A waypoint drag ended (fired just before [onWaypointMoved]): the host resumes re-solve. */
    onWaypointDragEnd: (Int) -> Unit = {},
    /** The pending route ORIGIN — the first point dropped (e.g. long-press "Start route here")
     *  before a destination exists, so there's no route yet. Drawn as a static origin pin that
     *  matches the committed waypoint[0] look, so it reads as "the route starts here" and doesn't
     *  visually change when the second point turns it into a real waypoint. */
    routeOrigin: LatLng? = null,
    /** Follow-mode checkpoints (position → crossed): filled+checked when passed, else outlined (US-3). */
    checkpoints: List<Pair<LatLng, Boolean>> = emptyList(),
) {
    val density = LocalDensity.current
    var viewportPx by remember { mutableStateOf(IntSize.Zero) }
    Box(modifier.fillMaxSize().onSizeChanged { viewportPx = it }) {
        // Live position — drawn first so it sits beneath markers/waypoints. Stands on the
        // terrain via the engine projection; the heading beam's on-screen angle is derived
        // from two projected points (position + a step along the heading), so it's correct
        // under map rotation and 3D tilt without the overlay needing the camera bearing.
        //
        // When the fix projects OUTSIDE the viewport — the common case on open, because the
        // app restores the last camera rather than recentring — we don't just place the dot
        // off-screen (invisible, the "I can't see it" bug). We clamp a directional chevron to
        // the screen edge pointing toward you, so your position is always discoverable (tap
        // the locate button to recentre).
        userLocation?.let { pos ->
            val boxPx = with(density) { 48.dp.toPx() }
            @Suppress("UNUSED_EXPRESSION") cameraTick
            val (sx, sy) = engine.toScreen(pos)
            val w = viewportPx.width.toFloat()
            val h = viewportPx.height.toFloat()
            val edgePadPx = with(density) { 26.dp.toPx() }
            // Onscreen iff the projected point is inside the viewport with a small inset (so a
            // dot half-off the edge still reads as the edge chevron). Until the first layout
            // pass sizes the box, treat as onscreen and let the offset lambda place it.
            val onScreen = w <= 0f || h <= 0f ||
                (sx in edgePadPx..(w - edgePadPx) && sy in edgePadPx..(h - edgePadPx))
            if (onScreen) {
                val screenHeading: Float? = userHeading?.let { h2 ->
                    val a = engine.toScreen(pos)
                    val b = engine.toScreen(aheadOf(pos, h2))
                    Math.toDegrees(atan2((b.second - a.second).toDouble(), (b.first - a.first).toDouble())).toFloat()
                }
                MyPositionPin(
                    screenHeadingDeg = screenHeading,
                    dotColor = userDotColor ?: UserBlue,
                    modifier = Modifier
                        .offset {
                            @Suppress("UNUSED_EXPRESSION") cameraTick
                            val (x, y) = engine.toScreen(pos)
                            IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx / 2f).roundToInt())
                        }
                        .testTag("myPosition"),
                )
            } else {
                // Clamp the chevron to where the centre→position ray exits the inset rect,
                // and rotate it to point along that ray.
                val cx = w / 2f
                val cy = h / 2f
                val dx = sx - cx
                val dy = sy - cy
                val hx = (cx - edgePadPx).coerceAtLeast(1f)
                val hy = (cy - edgePadPx).coerceAtLeast(1f)
                val scale = minOf(
                    if (dx != 0f) hx / kotlin.math.abs(dx) else Float.MAX_VALUE,
                    if (dy != 0f) hy / kotlin.math.abs(dy) else Float.MAX_VALUE,
                ).coerceAtMost(1f)
                val ex = cx + dx * scale
                val ey = cy + dy * scale
                val angle = Math.toDegrees(atan2(dy.toDouble(), dx.toDouble())).toFloat()
                OffScreenPositionChevron(
                    angleDeg = angle,
                    dotColor = userDotColor ?: UserBlue,
                    modifier = Modifier
                        .offset { IntOffset((ex - boxPx / 2f).roundToInt(), (ey - boxPx / 2f).roundToInt()) }
                        .testTag("myPositionOffscreen"),
                )
            }
        }
        checkpoints.forEach { (pos, crossed) ->
            val boxPx = with(density) { 22.dp.toPx() }
            CheckpointPin(
                crossed = crossed,
                modifier = Modifier.offset {
                    @Suppress("UNUSED_EXPRESSION") cameraTick
                    val (x, y) = engine.toScreen(pos)
                    IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx / 2f).roundToInt())
                },
            )
        }
        photoPins.forEach { pin ->
            val boxPx = with(density) { 56.dp.toPx() }
            Box(
                Modifier
                    .offset {
                        @Suppress("UNUSED_EXPRESSION") cameraTick
                        val (x, y) = engine.toScreen(LatLng(pin.lat, pin.lng))
                        IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx / 2f).roundToInt())
                    }
                    .testTag("photoPin"),
            ) {
                PhotoPinView(pin = pin, onClick = { onPhotoPinClick(pin) })
            }
        }
        markers.forEach { m ->
            val selected = m.id == selectedMarkerId
            val boxPx = with(density) { (if (selected) 42.dp else 33.dp).toPx() }
            MarkerPin(
                icon = m.kind.icon,
                selected = selected,
                color = m.colorArgb?.let { Color(it) } ?: markerFallbackColor,
                modifier = Modifier
                    .offset {
                        @Suppress("UNUSED_EXPRESSION") cameraTick
                        val (x, y) = engine.toScreen(m.position)
                        IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx).roundToInt())
                    }
                    .clickable { onMarkerClick(m) },
            )
        }
        // Editable route waypoints — drawn last so they sit above markers.
        waypoints.forEachIndexed { index, wp ->
            WaypointMarkerView(
                wp = wp,
                index = index,
                last = waypoints.lastIndex,
                selected = index == selectedWaypoint,
                cameraTick = cameraTick,
                project = { engine.toScreen(wp).let { Offset(it.first, it.second) } },
                // Commit a dragged waypoint via the TERRAIN raycast so the drop inverts the
                // terrain-lifted render projection ([toScreen]). The flat unproject can NOT be
                // used here: in 3D it intersects the z=0 plane below the relief, so the committed
                // point re-projected (lifted) lands at a different pixel — the drag-lands-wrong /
                // snap-back bug. (An early raycast over-shot at tilt, which is why this was once
                // flat; the first-hit cone-march fix made the raycast the correct inverse.)
                toGround = { o -> engine.screenToGround(o.x, o.y) },
                onTap = { onWaypointTap(index) },
                onLongPress = { onWaypointLongPress(index) },
                onDragStart = { onWaypointDragStart(index) },
                onMoved = { onWaypointMoved(index, it) },
                onDragEnd = { onWaypointDragEnd(index) },
            )
        }
        // Pending route origin (the first point, before a destination exists → no waypoints
        // yet). Same look as the committed waypoint[0] so it doesn't change on the second tap.
        routeOrigin?.let { o ->
            val boxPx = with(density) { 32.dp.toPx() }
            OriginPin(
                modifier = Modifier
                    .offset {
                        @Suppress("UNUSED_EXPRESSION") cameraTick
                        val (x, y) = engine.toScreen(o)
                        IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx / 2f).roundToInt())
                    }
                    .testTag("routeOrigin"),
            )
        }
    }
}

private val WpStart = Color(0xFF2E7D32)
private val WpEnd = Color(0xFFC0392B)

/** A static route-origin pin: the same green [WpStart] disc + [TripOrigin][Icons.Rounded.TripOrigin]
 *  badge as a committed waypoint[0], used for the pending origin before a route exists. */
@Composable
private fun OriginPin(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier.size(32.dp).clip(CircleShape).background(WpStart).border(3.dp, cs.surface, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.TripOrigin, null, tint = Color.White, modifier = Modifier.size(15.dp))
    }
}

/** A follow-mode checkpoint dot: a filled, checked circle once crossed, an outlined ring ahead. */
@Composable
private fun CheckpointPin(crossed: Boolean, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier.size(22.dp).clip(CircleShape)
            .background(if (crossed) cs.primary else cs.surface)
            .border(2.dp, cs.primary, CircleShape)
            .testTag(if (crossed) "checkpointCrossed" else "checkpointAhead"),
        contentAlignment = Alignment.Center,
    ) {
        if (crossed) {
            Icon(Icons.Rounded.Check, null, tint = Color.White, modifier = Modifier.size(14.dp))
        }
    }
}

/**
 * An on-map route waypoint: an A/B/C… letter badge (flag for the destination), start
 * green / end red / via primary, with a selected ring.
 *
 * Gestures: tap selects, long-press removes. Dragging picks the pin UP — a springy
 * Material-Expressive lift + scale + shadow — while a [DragLandingIndicator] marks the
 * exact ground point under it. On release it commits ONCE via [toGround] (the terrain
 * raycast, so it lands where it points over 3D relief) and then HOLDS its dropped screen
 * position until the parent's moved `wp` reprojects onto it — so the pin never blinks
 * back to its old spot between drop and state-commit ("no jump-back").
 *
 * The drag is tracked as an ABSOLUTE pointer position (not a delta accumulated off
 * `project()`), so the pin stays welded under the finger even if the camera pans mid-drag.
 */
@Composable
internal fun WaypointMarkerView(
    wp: LatLng,
    index: Int,
    last: Int,
    selected: Boolean,
    cameraTick: Int,
    project: () -> Offset,
    toGround: (Offset) -> LatLng,
    onTap: () -> Unit,
    onLongPress: () -> Unit,
    onDragStart: () -> Unit,
    onMoved: (LatLng) -> Unit,
    onDragEnd: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    val sizeDp = if (selected) 40.dp else 32.dp
    val boxPx = with(density) { sizeDp.toPx() }
    val color = when (index) {
        0 -> WpStart
        last -> WpEnd
        else -> cs.primary
    }

    // The gesture coroutine below is keyed on `index` only, so its closures live across
    // recompositions: they MUST read the current parameters through rememberUpdatedState.
    // Reading the raw parameters instead froze `wp` (via `project`) at first-composition
    // time — the SECOND drag of a pin then anchored at the pin's ORIGINAL position, landing
    // the drop wildly off (and a third drag "snapped back" to the first drop). The layout
    // offset follows recomposition, so the pin LOOKED right while every commit was wrong.
    val currentProject by rememberUpdatedState(project)
    val currentToGround by rememberUpdatedState(toGround)
    val currentOnMoved by rememberUpdatedState(onMoved)
    val currentOnDragStart by rememberUpdatedState(onDragStart)
    val currentOnDragEnd by rememberUpdatedState(onDragEnd)
    val currentOnTap by rememberUpdatedState(onTap)
    val currentOnLongPress by rememberUpdatedState(onLongPress)

    // Accumulated finger delta during a drag, HELD after drop until the committed waypoint
    // reprojects (no jump-back). Applied as a draw-only graphicsLayer translation — NOT a
    // layout offset. If the layout offset followed the drag, the box (which carries the
    // gesture detector) would move under the finger and every delta would be counted twice,
    // so the pin "doubled the movement and shot into the distance". Keeping the layout
    // anchored at project() holds the gesture's coordinate frame still.
    var dragOffset by remember { mutableStateOf(Offset.Zero) }
    var dragging by remember { mutableStateOf(false) }
    // The parent commits by changing `wp`: project() jumps to the drop point and we zero the
    // held translation in the same frame, so the pin lands seamlessly (no blink back).
    LaunchedEffect(wp) { if (!dragging) dragOffset = Offset.Zero }

    // The pin floats this far ABOVE its ground ring while dragging — a visual lift only (it's a
    // graphicsLayer translation, NOT part of the commit), so the raised pin is visible above the
    // fingertip while the ring marks the actual drop point under the finger. A fingertip contact
    // patch is ~48 dp and the pad of the finger hides more above it — 76 dp puts the lifted pin
    // clearly in view (52 dp still sat half-under the finger).
    val lift by animateDpAsState(
        if (dragging) 76.dp else 0.dp,
        spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "wpLift",
    )
    val scale by animateFloatAsState(
        if (dragging) 1.25f else 1f,
        spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "wpScale",
    )
    val liftPx = with(density) { lift.toPx() }
    val showLanding = dragging || lift > 0.dp

    Box(
        Modifier
            .offset {
                // STABLE layout anchor: only the camera (cameraTick) moves it, never the drag.
                @Suppress("UNUSED_EXPRESSION") cameraTick
                val p = project()
                IntOffset((p.x - boxPx / 2f).roundToInt(), (p.y - boxPx / 2f).roundToInt())
            }
            .size(sizeDp)
            .testTag("waypoint_$index")
            .pointerInput(index) {
                detectDragGestures(
                    onDragStart = {
                        // ZERO every drag (not +=): start fresh so repeated drops can't accumulate
                        // drift ("placed way too far in the same direction"). The ring then tracks
                        // the finger 1:1 and commits exactly under it.
                        dragOffset = Offset.Zero
                        dragging = true
                        currentOnDragStart()
                    },
                    onDrag = { change, amount ->
                        change.consume()
                        dragOffset += amount
                    },
                    onDragEnd = {
                        val drop = currentProject() + dragOffset
                        dragging = false
                        // End the drag session BEFORE committing so the move's re-solve isn't
                        // swallowed by the suppression; keep `dragOffset` until `wp` updates so
                        // the pin holds the drop spot (no jump-back).
                        currentOnDragEnd()
                        currentOnMoved(currentToGround(drop))
                    },
                    onDragCancel = {
                        dragging = false
                        dragOffset = Offset.Zero
                        currentOnDragEnd()
                    },
                )
            }
            .pointerInput(index) {
                detectTapGestures(onTap = { currentOnTap() }, onLongPress = { currentOnLongPress() })
            },
        contentAlignment = Alignment.Center,
    ) {
        // Landing target at the drop ground point (anchor + drag), drawn with the same drag
        // translation but NO lift — the pin floats above it. The Box doesn't clip, so this and
        // the lifted pin draw beyond the sizeDp bounds.
        if (showLanding) {
            DragLandingIndicator(
                color = color,
                modifier = Modifier.graphicsLayer {
                    translationX = dragOffset.x
                    translationY = dragOffset.y
                },
            )
        }
        Box(
            Modifier.graphicsLayer {
                translationX = dragOffset.x
                translationY = dragOffset.y - liftPx
                scaleX = scale
                scaleY = scale
            },
            contentAlignment = Alignment.Center,
        ) {
            if (selected) {
                Box(Modifier.size(sizeDp + 18.dp).clip(CircleShape).background(color.copy(alpha = 0.20f)))
            }
            Box(
                Modifier
                    .size(sizeDp)
                    .then(if (showLanding) Modifier.shadow(10.dp, CircleShape) else Modifier)
                    .clip(CircleShape)
                    .background(color)
                    .border(3.dp, cs.surface, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                when {
                    // Origin = a "trip origin" ring, destination = a flag, the stops between
                    // are lettered A, B, C… (the first via is A; origin/dest carry icons).
                    index == 0 -> Icon(
                        Icons.Rounded.TripOrigin, null, tint = Color.White,
                        modifier = Modifier.size(if (selected) 18.dp else 15.dp),
                    )
                    index == last -> Icon(
                        Icons.Rounded.Flag, null, tint = Color.White,
                        modifier = Modifier.size(if (selected) 20.dp else 16.dp),
                    )
                    else -> Text(
                        ('A' + (index - 1)).toString(),
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W800),
                        color = Color.White,
                    )
                }
            }
        }
    }
}

/**
 * The "where it'll land" target shown under a lifted waypoint: a pulsing ring plus a solid
 * centre dot marking the exact ground point the drop will commit to. Purely decorative —
 * no input — and centred on the drag anchor. Sized to read AROUND a fingertip (~48 dp
 * contact patch): a 56 dp ring stays visible as a rim while the finger covers its centre,
 * and the white under-stroke keeps it legible on any basemap colour.
 */
@Composable
private fun DragLandingIndicator(color: Color, modifier: Modifier = Modifier) {
    val pulse = rememberInfiniteTransition(label = "wpLanding")
    val t by pulse.animateFloat(
        initialValue = 0.78f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(750), RepeatMode.Reverse),
        label = "wpLandingPulse",
    )
    Canvas(modifier.size(56.dp)) {
        val r = size.minDimension / 2f
        drawCircle(color = color.copy(alpha = 0.18f), radius = r * t, center = center)
        // White halo under the coloured ring — contrast against dark AND light ground.
        drawCircle(color = Color.White.copy(alpha = 0.9f), radius = r * 0.92f * t, center = center, style = Stroke(width = 9f))
        drawCircle(color = color, radius = r * 0.92f * t, center = center, style = Stroke(width = 5f))
        drawCircle(color = Color.White, radius = r * 0.14f + 3f, center = center)
        drawCircle(color = color, radius = r * 0.14f, center = center)
    }
}

private val UserBlue = Color(0xFF1A73E8)

/**
 * The live device-position dot — a white-haloed blue disc that sits on the terrain. When
 * [screenHeadingDeg] is non-null (the fix carries a course) it grows a translucent heading
 * beam; the angle is already in SCREEN space (the caller derives it from the engine
 * projection), so it stays correct as the map rotates or tilts.
 */
@Composable
private fun MyPositionPin(screenHeadingDeg: Float?, modifier: Modifier = Modifier, dotColor: Color = UserBlue) {
    Canvas(modifier.size(48.dp)) {
        val c = center
        val dot = 7.dp.toPx()
        if (screenHeadingDeg != null) {
            rotate(screenHeadingDeg, pivot = c) {
                val len = 20.dp.toPx()
                val half = 9.dp.toPx()
                val beam = Path().apply {
                    moveTo(c.x, c.y - half)
                    lineTo(c.x + len, c.y)
                    lineTo(c.x, c.y + half)
                    close()
                }
                drawPath(
                    beam,
                    brush = Brush.horizontalGradient(
                        0f to dotColor.copy(alpha = 0.55f),
                        1f to dotColor.copy(alpha = 0f),
                        startX = c.x,
                        endX = c.x + len,
                    ),
                )
            }
        }
        drawCircle(Color.White, radius = dot + 3.dp.toPx(), center = c)
        drawCircle(dotColor, radius = dot, center = c)
    }
}

/**
 * Edge indicator for when the live position is OFF-SCREEN: a white-ringed blue disc with an
 * arrowhead pointing toward your (off-frame) location. [angleDeg] is the screen-space direction
 * from the viewport centre to the position (0 = +x / right, clockwise), so it stays correct under
 * rotation. The caller clamps it to the screen edge; tapping the locate button recentres the map.
 */
@Composable
private fun OffScreenPositionChevron(angleDeg: Float, modifier: Modifier = Modifier, dotColor: Color = UserBlue) {
    Canvas(modifier.size(48.dp)) {
        val c = center
        val dot = 6.dp.toPx()
        rotate(angleDeg, pivot = c) {
            val tip = c.x + 18.dp.toPx()
            val baseX = c.x + 8.dp.toPx()
            val half = 8.dp.toPx()
            val arrow = Path().apply {
                moveTo(tip, c.y)
                lineTo(baseX, c.y - half)
                lineTo(baseX, c.y + half)
                close()
            }
            drawPath(arrow, color = dotColor)
        }
        drawCircle(Color.White, radius = dot + 3.dp.toPx(), center = c)
        drawCircle(dotColor, radius = dot, center = c)
    }
}

/**
 * A point ~20 m from [p] along compass [headingDeg] (0 = N, clockwise). Used only to derive the
 * heading beam's on-screen direction by projecting both points through the engine — a small
 * flat-earth step, exact enough at this distance for a screen-space angle.
 */
private fun aheadOf(p: LatLng, headingDeg: Float): LatLng {
    val h = Math.toRadians(headingDeg.toDouble())
    val dDeg = 0.00018 // ~20 m in latitude degrees
    val dLat = dDeg * cos(h)
    val dLng = dDeg * sin(h) / cos(Math.toRadians(p.lat)).coerceAtLeast(1e-6)
    return LatLng(p.lat + dLat, p.lng + dLng)
}

/** White-framed rounded thumbnail with a count badge for clusters. */
@Composable
private fun PhotoPinView(pin: PhotoPin, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(Modifier.size(56.dp)) {
        Surface(
            onClick = onClick,
            shape = RoundedCornerShape(16.dp),
            color = cs.surface,
            shadowElevation = 6.dp,
            modifier = Modifier.size(56.dp),
        ) {
            Box(Modifier.fillMaxSize().padding(3.dp).clip(RoundedCornerShape(13.dp)).background(cs.surfaceContainerHighest)) {
                val bmp = pin.coverPath?.let { rememberThumb(it) }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            }
        }
        if (pin.count > 1) {
            Surface(
                shape = CircleShape,
                color = cs.primary,
                shadowElevation = 2.dp,
                modifier = Modifier.align(Alignment.TopEnd).offset(x = 6.dp, y = (-6).dp),
            ) {
                Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
                    Text(
                        "${pin.count}",
                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
                        color = cs.onPrimary,
                    )
                }
            }
        }
    }
}

/** Decodes [path] off the main thread, downsampled for an on-map thumbnail. */
@Composable
private fun rememberThumb(path: String): ImageBitmap? {
    val state by produceState<ImageBitmap?>(initialValue = null, path) {
        value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            runCatching {
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(path, bounds)
                var sample = 1
                while (bounds.outWidth / sample > 160 || bounds.outHeight / sample > 160) sample *= 2
                BitmapFactory.decodeFile(path, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
            }.getOrNull()
        }
    }
    return state
}
