package com.sigmundgranaas.turbo.expressive.feature.map.radar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.WeatherCloudOverlay
import kotlinx.coroutines.delay
import kotlin.math.cos

/**
 * Self-contained weather-radar overlay control: a toggle that, when on, loads a
 * radar sequence, drives the GPU cloud overlay, and shows a play/scrub time
 * slider. Renders nothing if the active [engine] can't draw clouds (e.g.
 * MapLibre), so the call site stays a one-liner.
 *
 * The [source] defaults to the offline synthetic storm; swap in
 * [MetRadarDataSource] once its georeferencing is wired.
 */
@Composable
fun RadarOverlayControls(
    engine: MapEngine?,
    active: Boolean,
    onActiveChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    source: RadarDataSource = SyntheticRadarDataSource(),
) {
    val eng = engine ?: return
    val overlay = eng as? WeatherCloudOverlay ?: return
    val state = remember(overlay) { RadarOverlayState(overlay) }
    // The geo box the loaded radar covers. A fixed-span box (not the viewport)
    // so the procedural cloud field is world-locked: zooming shows fewer, bigger
    // puffs rather than a screen-fixed texture. Reloaded when the camera pans
    // out of it (below).
    var loadedBox by remember(overlay) { mutableStateOf<GeoBounds?>(null) }

    // Load (and re-load) the sequence: first time on, then whenever the camera
    // pans far enough that the current box no longer comfortably covers the view.
    LaunchedEffect(active, eng) {
        if (!active) {
            state.setVisible(false)
            return@LaunchedEffect
        }
        while (true) {
            val view = eng.visibleBounds()
            if (loadedBox == null || !boxStillGood(loadedBox!!, view)) {
                val box = fixedBoxAround(view)
                state.loadFrames(source.load(box, frameCount = 12), box)
                loadedBox = box
            }
            state.setVisible(state.ready)
            delay(RELOAD_POLL_MS) // cheap re-check of the camera box
        }
    }

    // Playback: advance the timeline each frame while playing.
    LaunchedEffect(state.playing, active) {
        if (!active || !state.playing) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            state.advance((now - last) / 1_000_000_000f)
            last = now
        }
    }

    // Enabling clouds is a Layers-menu option now (not a chip on the map), so
    // when off this renders nothing; the call site places the scrubber at the
    // bottom of the screen.
    if (!active) return

    Surface(
        modifier = modifier.widthIn(max = 520.dp),
        shape = MaterialTheme.shapes.large,
        tonalElevation = 3.dp,
    ) {
        Row(
            Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            IconButton(onClick = { state.togglePlay() }, enabled = state.ready) {
                if (state.playing) {
                    Icon(Icons.Filled.Pause, contentDescription = "Pause")
                } else {
                    Icon(Icons.Filled.PlayArrow, contentDescription = "Play")
                }
            }
            Slider(
                value = state.position,
                onValueChange = { state.seek(it) },
                valueRange = 0f..(state.frameCount - 1).coerceAtLeast(1).toFloat(),
                enabled = state.ready,
                modifier = Modifier.weight(1f),
            )
            Text(
                state.currentLabel(),
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.widthIn(min = 44.dp),
            )
            IconButton(onClick = { onActiveChange(false) }) {
                Icon(Icons.Filled.Close, contentDescription = "Hide radar")
            }
        }
    }
}

/** How often to re-check whether the loaded box still suits the camera. */
private const val RELOAD_POLL_MS = 600L

/** The loaded box is sized to this multiple of the *view* half-extent, so the
 *  on-screen puff count (≈ field-frequency · view/box) stays constant at every
 *  zoom. A FIXED-degree box (the old approach) made the box ~330 km, so zooming
 *  in to hiking range put the whole screen inside one puff → a flat white wash.
 *  Bigger margin → more pan room before a reload, but fewer puffs on screen. */
private const val BOX_VIEW_MARGIN = 6.0

/** Clamp the box half-height (deg lat) so it's neither absurdly tiny at max zoom
 *  nor continent-spanning when zoomed right out. */
private const val MIN_HALF_LAT_DEG = 0.03
private const val MAX_HALF_LAT_DEG = 3.0

/**
 * A geo box centred on [view], sized [BOX_VIEW_MARGIN]× the view so puffs stay
 * screen-sized at any zoom. Square on the ground (lng half = lat half / cos lat)
 * so the procedural puffs come out round, not stretched.
 */
private fun fixedBoxAround(view: GeoBounds): GeoBounds {
    val cLat = (view.south + view.north) / 2.0
    val cLng = (view.west + view.east) / 2.0
    val viewHalfLat = (view.north - view.south) / 2.0
    val cosLat = cos(Math.toRadians(cLat)).coerceAtLeast(0.2)
    val halfLat = (viewHalfLat * BOX_VIEW_MARGIN).coerceIn(MIN_HALF_LAT_DEG, MAX_HALF_LAT_DEG)
    val halfLng = halfLat / cosLat
    return GeoBounds(
        south = cLat - halfLat,
        west = cLng - halfLng,
        north = cLat + halfLat,
        east = cLng + halfLng,
    )
}

/**
 * Whether the loaded [box] still suits [view] — no reload needed. Reload when the
 * camera has panned out of the box (its old job) OR when zoom has drifted far
 * enough that the box no longer matches the view: too big (zoomed in) re-creates
 * the white wash, too small (zoomed out) loses pan margin. The accepted band is
 * `[MARGIN/2, MARGIN·2]`, so you can zoom ~1 level either way before a reload
 * re-tightens — which is what makes the clouds scale as you zoom.
 */
private fun boxStillGood(box: GeoBounds, view: GeoBounds): Boolean {
    val inside = view.south > box.south && view.north < box.north &&
        view.west > box.west && view.east < box.east
    if (!inside) return false
    val boxHalfLat = (box.north - box.south) / 2.0
    val viewHalfLat = ((view.north - view.south) / 2.0).coerceAtLeast(1e-9)
    val ratio = boxHalfLat / viewHalfLat
    return ratio >= BOX_VIEW_MARGIN * 0.5 && ratio <= BOX_VIEW_MARGIN * 2.0
}
