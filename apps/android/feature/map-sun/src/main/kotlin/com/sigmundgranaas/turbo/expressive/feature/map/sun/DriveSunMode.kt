package com.sigmundgranaas.turbo.expressive.feature.map.sun

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.TerrainSunOverlay
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId

/**
 * Drives "sun mode" on the wgpu terrain from the layers-sheet Sun slider.
 *
 * The slider's value is reduced to a sun position ([sunHour], an hour-of-day) by
 * `mapEnvironment`; this effect rakes the engine's sun + cast shadows to that
 * position. Null = the sun is off, which clears the shadows. Sun position never
 * touches the camera — lighting the relief works in 2D (top-down) and 3D alike.
 *
 * Headless: renders nothing (it's an effect), and no-ops on an engine that can't
 * sun-light terrain (e.g. MapLibre), so the call site stays a one-liner.
 */
@Composable
fun DriveSunMode(engine: MapEngine?, sunHour: Float?) {
    val sun = engine as? TerrainSunOverlay ?: return

    // Rake the sun to the slider position + raise cast shadows; clear when off.
    LaunchedEffect(sun, sunHour) {
        if (sunHour != null) {
            sun.setSunTime(unixForHourToday(sunHour))
            sun.setTerrainShadows(SUN_MODE_SHADOW_STRENGTH)
        } else {
            sun.setTerrainShadows(0f)
        }
    }
    // Don't leave shadows burned into the terrain if the control leaves composition.
    DisposableEffect(sun) {
        onDispose { sun.setTerrainShadows(0f) }
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
