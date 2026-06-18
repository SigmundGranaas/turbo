package com.sigmundgranaas.turbo.expressive.feature.map.radar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.FilledTonalButton
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
import kotlin.math.max

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
    modifier: Modifier = Modifier,
    source: RadarDataSource = SyntheticRadarDataSource(),
) {
    val eng = engine ?: return
    val overlay = eng as? WeatherCloudOverlay ?: return
    val state = remember(overlay) { RadarOverlayState(overlay) }
    var active by remember(overlay) { mutableStateOf(false) }
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
            if (loadedBox == null || !boxCovers(loadedBox!!, view)) {
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

    if (!active) {
        FilledTonalButton(onClick = { active = true }, modifier = modifier) {
            Text("Radar")
        }
        return
    }

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
            IconButton(onClick = { active = false }) {
                Icon(Icons.Filled.Close, contentDescription = "Hide radar")
            }
        }
    }
}

/** How often to re-check whether the camera has left the loaded radar box. */
private const val RELOAD_POLL_MS = 600L

/** Minimum half-extent of the loaded box (degrees of latitude). Big enough that
 *  at typical hiking zooms the box is FIXED in the world, so the cloud field is
 *  world-locked (zooming shows fewer, bigger puffs). */
private const val BASE_HALF_LAT_DEG = 1.5

/**
 * A geo box centred on [view] that the radar is loaded for. At least
 * [BASE_HALF_LAT_DEG] half-height, and at least 1.6× the view so there's pan
 * margin before a reload; when zoomed far out it grows with the view. The
 * longitude half-span is widened by 1/cos(lat) so the box is ~square on the
 * ground (and so the procedural puffs come out roughly round, not stretched).
 */
private fun fixedBoxAround(view: GeoBounds): GeoBounds {
    val cLat = (view.south + view.north) / 2.0
    val cLng = (view.west + view.east) / 2.0
    val viewHalfLat = (view.north - view.south) / 2.0
    val viewHalfLng = (view.east - view.west) / 2.0
    val cosLat = cos(Math.toRadians(cLat)).coerceAtLeast(0.2)
    val halfLat = max(BASE_HALF_LAT_DEG, viewHalfLat * 1.6)
    val halfLng = max(BASE_HALF_LAT_DEG / cosLat, viewHalfLng * 1.6)
    return GeoBounds(
        south = cLat - halfLat,
        west = cLng - halfLng,
        north = cLat + halfLat,
        east = cLng + halfLng,
    )
}

/** Whether [box] still strictly contains [view] — i.e. no reload needed yet. */
private fun boxCovers(box: GeoBounds, view: GeoBounds): Boolean =
    view.south > box.south && view.north < box.north &&
        view.west > box.west && view.east < box.east
