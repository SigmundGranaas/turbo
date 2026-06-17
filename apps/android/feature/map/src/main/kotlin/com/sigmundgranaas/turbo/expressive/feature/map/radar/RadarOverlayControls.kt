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
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.WeatherCloudOverlay

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

    // Load the sequence the first time the overlay is switched on.
    LaunchedEffect(active, eng) {
        if (active && !state.ready) {
            state.setFrames(source.load(eng.visibleBounds(), frameCount = 12))
        }
        state.setVisible(active && state.ready)
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
