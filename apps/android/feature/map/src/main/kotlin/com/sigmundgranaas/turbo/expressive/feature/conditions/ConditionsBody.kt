package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.WindArrow
import com.sigmundgranaas.turbo.expressive.ui.components.weatherIcon
import androidx.compose.material3.Icon
import com.sigmundgranaas.turbo.expressive.ui.theme.DangerColors
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.roundToInt

/**
 * Live "conditions now" for the selected point: current weather from MET Norway,
 * plus today's Varsom avalanche danger when available. Rendered inside the map
 * selection detail sheet.
 */
@Composable
fun ConditionsBody(point: LatLng, viewModel: ConditionsViewModel = hiltViewModel()) {
    val cs = MaterialTheme.colorScheme
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(point) { viewModel.load(point) }

    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh).padding(18.dp),
    ) {
        SectionLabel("Conditions now · yr.no")
        Spacer(Modifier.height(14.dp))
        when (val s = state) {
            is ConditionsUiState.Loading -> Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(modifier = Modifier.size(28.dp))
            }
            is ConditionsUiState.Error -> Text(
                "Conditions unavailable offline.",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
            is ConditionsUiState.Content -> {
                s.conditions.weather?.let { WeatherTiles(it) }
                s.conditions.avalanche?.let {
                    Spacer(Modifier.height(12.dp))
                    AvalancheRow(it)
                }
                if (s.conditions.weather == null && s.conditions.avalanche == null) {
                    Text("No data for this location.", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
private fun WeatherTiles(weather: WeatherNow) {
    val cs = MaterialTheme.colorScheme
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        // Hero: condition symbol + temperature, with wind direction on the right.
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(weatherIcon(weather.symbolCode), null, tint = cs.primary, modifier = Modifier.size(38.dp))
            Spacer(Modifier.size(12.dp))
            Text(
                weather.temperatureC?.let { "${it.roundToInt()}°" } ?: "—",
                style = MaterialTheme.typography.displaySmall.copy(fontWeight = FontWeight.W700),
                color = cs.onSurface,
            )
            Spacer(Modifier.weight(1f))
            WindArrow(weather.windFromDeg, Modifier.size(20.dp))
            Spacer(Modifier.size(6.dp))
            Text(
                buildString {
                    append(weather.windSpeedMs?.let { "${it.roundToInt()} m/s" } ?: "—")
                    compass(weather.windFromDeg).takeIf { it.isNotEmpty() }?.let { append(" $it") }
                },
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Tile(weather.precipitationMm?.let { "%.1f".format(it) } ?: "0.0", "mm/h", Modifier.weight(1f))
            Tile(weather.humidityPct?.let { "${it.roundToInt()}%" } ?: "—", "Humidity", Modifier.weight(1f))
            Tile(weather.cloudCoverPct?.let { "${it.roundToInt()}%" } ?: "—", "Cloud", Modifier.weight(1f))
            Tile(weather.uvIndex?.let { "${it.roundToInt()}" } ?: "—", "UV", Modifier.weight(1f))
        }
    }
}

@Composable
private fun Tile(value: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier.clip(RoundedCornerShape(TurboRadius.m)).background(cs.surfaceContainerLowest).padding(vertical = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(value, style = MaterialTheme.typography.titleLarge, color = cs.onSurface)
            Text(label, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
        }
    }
}

@Composable
private fun AvalancheRow(avalanche: AvalancheNow) {
    val cs = MaterialTheme.colorScheme
    val danger = DangerColors.all[(avalanche.dangerLevel - 1).coerceIn(0, 4)]
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.m)).background(danger.copy(alpha = 0.14f)).padding(12.dp),
    ) {
        Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).background(danger), contentAlignment = Alignment.Center) {
            Text("${avalanche.dangerLevel}", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800), color = androidx.compose.ui.graphics.Color.White)
        }
        Spacer(Modifier.size(12.dp))
        Column {
            Text("Avalanche · Level ${avalanche.dangerLevel}", style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
            Text(avalanche.region.ifBlank { "Varsom" }, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
        }
    }
}

private fun compass(deg: Double?): String {
    if (deg == null) return ""
    val dirs = listOf("N", "NE", "E", "SE", "S", "SW", "W", "NW")
    return dirs[(((deg % 360) / 45).roundToInt()) % 8]
}
