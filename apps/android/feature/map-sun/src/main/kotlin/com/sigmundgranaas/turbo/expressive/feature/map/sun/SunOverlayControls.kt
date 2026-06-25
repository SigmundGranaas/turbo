package com.sigmundgranaas.turbo.expressive.feature.map.sun

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.WbSunny
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.TerrainSunOverlay
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId

/**
 * "Sun mode": when active, lights the 3D terrain by a movable sun — the
 * analytic sky/atmosphere, the time-of-day shading, and the cast shadows (a
 * peak shadowing the valley). A bottom slider scrubs the hour of *today*
 * (defaults to the current time); dragging it moves the sun, so you can rake
 * long dawn/dusk shadows across the relief.
 *
 * Renders nothing if the engine can't sun-light terrain (e.g. MapLibre), so the
 * call site stays a one-liner — same shape as the radar overlay control.
 */
@Composable
fun SunOverlayControls(
    engine: MapEngine?,
    active: Boolean,
    onActiveChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    val sun = engine as? TerrainSunOverlay ?: return

    // Hour-of-day in [0, 24), defaulting to now. Persisted across toggles within
    // a single composition so re-enabling resumes where the user left off.
    var hour by remember {
        val now = LocalTime.now()
        mutableFloatStateOf(now.hour + now.minute / 60f)
    }

    // Drive the sun whenever active + the hour changes; clear shadows when off.
    // `unixForHourToday` keeps the date = today so the solar elevation is
    // realistic for the season at the map's latitude.
    LaunchedEffect(active, hour) {
        if (active) {
            sun.setSunTime(unixForHourToday(hour))
            sun.setTerrainShadows(SUN_MODE_SHADOW_STRENGTH)
        }
    }
    // Turn shadows off when the control leaves composition or is toggled off,
    // so cast shadows don't linger after the user exits sun mode.
    DisposableEffect(active) {
        onDispose {
            if (!active) sun.setTerrainShadows(0f)
        }
    }
    LaunchedEffect(active) {
        if (!active) sun.setTerrainShadows(0f)
    }

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
            Icon(Icons.Filled.WbSunny, contentDescription = "Sun", modifier = Modifier.padding(start = 4.dp))
            Slider(
                value = hour,
                onValueChange = { hour = it },
                valueRange = 0f..24f,
                modifier = Modifier.weight(1f),
            )
            Text(
                clockLabel(hour),
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.widthIn(min = 44.dp),
            )
            IconButton(onClick = { onActiveChange(false) }) {
                Icon(Icons.Filled.Close, contentDescription = "Exit sun mode")
            }
        }
    }
}

/** Cast-shadow strength used while sun mode is on — strong enough to read on relief. */
private const val SUN_MODE_SHADOW_STRENGTH = 0.85f

/** UTC seconds for today's date at local `hour` (fractional). The engine solves
 *  the sun's azimuth/altitude from this instant at the camera location. */
private fun unixForHourToday(hour: Float): Double {
    val zone = ZoneId.systemDefault()
    val h = hour.coerceIn(0f, 23.999f)
    val time = LocalTime.of(h.toInt(), ((h - h.toInt()) * 60).toInt().coerceIn(0, 59))
    return LocalDate.now().atTime(time).atZone(zone).toEpochSecond().toDouble()
}

/** "HH:MM" for the slider readout. */
private fun clockLabel(hour: Float): String {
    val h = hour.toInt().coerceIn(0, 23)
    val m = ((hour - hour.toInt()) * 60).toInt().coerceIn(0, 59)
    return "%02d:%02d".format(h, m)
}
