package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bookmark
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.roundToInt

/** Bottom card reflecting the route solve: presets, solving spinner, result stats, follow, or error. */
@Composable
internal fun RouteCard(
    state: RouteUiState,
    preset: RoutePreset,
    userLocation: LatLng?,
    onSelectPreset: (RoutePreset) -> Unit,
    onFollow: () -> Unit,
    onSave: () -> Unit,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (state is RouteUiState.Idle) return
    val cs = MaterialTheme.colorScheme
    val metric = LocalMetricUnits.current
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(TurboRadius.xl),
        color = cs.surfaceContainerHigh,
        shadowElevation = 4.dp,
    ) {
        Column(Modifier.padding(horizontal = 18.dp, vertical = 14.dp)) {
            when (state) {
                is RouteUiState.Solving -> {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.5.dp)
                        Spacer(Modifier.width(14.dp))
                        Text("Finding the best route…", style = MaterialTheme.typography.titleMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                        TextButton(onClick = onClear) { Text("Cancel") }
                    }
                    PresetRow(preset, onSelectPreset)
                }
                is RouteUiState.Error -> Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(state.message, style = MaterialTheme.typography.bodyMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                    TextButton(onClick = onClear) { Text("Dismiss") }
                }
                is RouteUiState.Done -> {
                    val p = state.plan
                    Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        RouteStat(Units.distance(p.distanceM, metric), "Distance")
                        RouteStat(formatDuration(p.durationS), "Time")
                        RouteStat(Units.elevation(p.ascentM, metric), "Ascent")
                        RouteStat("${p.onTrailPct.roundToInt()}%", "On trail")
                    }
                    SurfaceBreakdown(p.surfaces)
                    PresetRow(preset, onSelectPreset)
                    Spacer(Modifier.height(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Button(onClick = onFollow, modifier = Modifier.weight(1f)) {
                            Icon(Icons.Rounded.Navigation, null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(8.dp))
                            Text("Follow")
                        }
                        FilledTonalButton(onClick = onSave) {
                            Icon(Icons.Rounded.Bookmark, null, modifier = Modifier.size(18.dp))
                        }
                        TextButton(onClick = onClear) { Text("Clear") }
                    }
                }
                is RouteUiState.Following -> {
                    val progress = userLocation?.let { GeoMetrics.progress(state.plan.geometry, it, state.plan.ascentM) }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.Navigation, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text("Following route", style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
                            Text(
                                text = progress?.let {
                                    "${Units.distance(it.distanceRemainingM, metric)} left" +
                                        (it.etaSeconds?.let { s -> " · ${formatDuration(s.toDouble())}" } ?: "")
                                } ?: "Waiting for GPS…",
                                style = MaterialTheme.typography.bodyMedium,
                                color = cs.onSurfaceVariant,
                            )
                        }
                        TextButton(onClick = onClear) { Text("Stop") }
                    }
                }
                RouteUiState.Idle -> Unit
            }
        }
    }
}

@Composable
private fun PresetRow(selected: RoutePreset, onSelect: (RoutePreset) -> Unit) {
    val cs = MaterialTheme.colorScheme
    Spacer(Modifier.height(10.dp))
    Row(
        Modifier.horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        RoutePreset.entries.forEach { p ->
            FilterChip(
                selected = p == selected,
                onClick = { onSelect(p) },
                label = { Text(p.label) },
                leadingIcon = { Icon(p.icon, null, Modifier.size(18.dp)) },
            )
        }
    }
    Spacer(Modifier.height(6.dp))
    Text(selected.description, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
}

/** Proportional trail/road/other surface bar derived from the plan's per-surface metres. */
@Composable
private fun SurfaceBreakdown(surfaces: Map<String, Double>) {
    val cs = MaterialTheme.colorScheme
    val total = surfaces.values.sum()
    if (total <= 0.0) return
    Spacer(Modifier.height(12.dp))
    Row(
        Modifier.fillMaxWidth().height(8.dp).clip(RoundedCornerShape(4.dp)),
    ) {
        surfaces.entries.sortedByDescending { it.value }.forEach { (surface, meters) ->
            val fraction = (meters / total).toFloat()
            if (fraction <= 0f) return@forEach
            Spacer(
                Modifier.weight(fraction).fillMaxWidth().height(8.dp).background(surfaceColor(surface, cs)),
            )
        }
    }
    Spacer(Modifier.height(6.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        surfaces.entries.sortedByDescending { it.value }.take(4).forEach { (surface, meters) ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Spacer(Modifier.size(8.dp).clip(RoundedCornerShape(2.dp)).background(surfaceColor(surface, cs)))
                Spacer(Modifier.width(5.dp))
                Text(
                    "${surface.replaceFirstChar(Char::uppercase)} ${((meters / total) * 100).roundToInt()}%",
                    style = MaterialTheme.typography.labelSmall,
                    color = cs.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun surfaceColor(surface: String, cs: androidx.compose.material3.ColorScheme) = when (surface.lowercase()) {
    "trail" -> cs.primary
    "road" -> cs.tertiary
    "ski" -> cs.secondary
    else -> cs.outline
}

@Composable
private fun RouteStat(value: String, label: String) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(value, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

private fun formatDuration(seconds: Double): String {
    val total = seconds.roundToInt()
    val h = total / 3600
    val m = (total % 3600) / 60
    return if (h > 0) "${h}h ${m}m" else "$m min"
}
