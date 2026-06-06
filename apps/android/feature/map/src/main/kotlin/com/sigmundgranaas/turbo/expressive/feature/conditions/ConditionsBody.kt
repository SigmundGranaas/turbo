package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.res.stringResource
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
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
    var showForecast by remember { mutableStateOf(false) }
    LaunchedEffect(point) { viewModel.load(point) }

    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh).padding(18.dp),
    ) {
        SectionLabel(stringResource(R.string.cond_header))
        Spacer(Modifier.height(14.dp))
        when (val s = state) {
            is ConditionsUiState.Loading -> Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(modifier = Modifier.size(28.dp))
            }
            is ConditionsUiState.Error -> Text(
                stringResource(R.string.cond_unavailable),
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
            is ConditionsUiState.Content -> {
                s.conditions.weather?.let { WeatherTiles(it) }
                s.conditions.avalanche?.let {
                    Spacer(Modifier.height(12.dp))
                    AvalancheRow(it)
                }
                s.conditions.marine?.let {
                    Spacer(Modifier.height(12.dp))
                    MarineRow(it)
                }
                if (s.conditions.weather == null && s.conditions.avalanche == null) {
                    Text(stringResource(R.string.cond_no_data), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                } else if (s.conditions.weather != null) {
                    Spacer(Modifier.height(14.dp))
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.m))
                            .clickable { showForecast = true }.padding(vertical = 8.dp),
                    ) {
                        Text(stringResource(R.string.cond_full_forecast), style = MaterialTheme.typography.titleSmall, color = cs.primary, modifier = Modifier.weight(1f))
                        Icon(Icons.AutoMirrored.Rounded.ArrowForward, null, tint = cs.primary, modifier = Modifier.size(18.dp))
                    }
                }
            }
        }
    }

    if (showForecast) {
        WeatherForecastSheet(point = point, onDismiss = { showForecast = false }, viewModel = viewModel)
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
            Tile(weather.humidityPct?.let { "${it.roundToInt()}%" } ?: "—", stringResource(R.string.cond_humidity), Modifier.weight(1f))
            Tile(weather.cloudCoverPct?.let { "${it.roundToInt()}%" } ?: "—", stringResource(R.string.cond_cloud), Modifier.weight(1f))
            Tile(weather.uvIndex?.let { "${it.roundToInt()}" } ?: "—", stringResource(R.string.cond_uv), Modifier.weight(1f))
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
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.m)).background(danger.copy(alpha = 0.14f)).padding(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(36.dp).clip(RoundedCornerShape(TurboRadius.s)).background(danger), contentAlignment = Alignment.Center) {
                Text("${avalanche.dangerLevel}", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800), color = androidx.compose.ui.graphics.Color.White)
            }
            Spacer(Modifier.size(12.dp))
            Column {
                Text(stringResource(R.string.cond_avalanche_level, avalanche.dangerLevel), style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                Text(avalanche.region.ifBlank { "Varsom" }, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }
        avalanche.problems.forEach { p ->
            val parts = listOfNotNull(p.type, p.trigger, p.size)
            if (parts.isNotEmpty()) {
                Spacer(Modifier.height(8.dp))
                Text("• ${parts.joinToString(" · ")}", style = MaterialTheme.typography.bodySmall, color = cs.onSurface)
            }
        }
    }
}

@Composable
private fun MarineRow(marine: MarineNow) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.m)).background(cs.surfaceContainerLowest).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("Sea", style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
            Text(
                buildString {
                    marine.seaTemperatureC?.let { append("${it.roundToInt()}° water") }
                    if (marine.seaTemperatureC != null && marine.waveHeightM != null) append(" · ")
                    marine.waveHeightM?.let { append("%.1f m waves".format(it)) }
                },
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        }
        if (marine.waveFromDeg != null) {
            WindArrow(marine.waveFromDeg, Modifier.size(20.dp))
            Spacer(Modifier.size(6.dp))
            Text(compass(marine.waveFromDeg), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
        }
    }
}

private fun compass(deg: Double?): String {
    if (deg == null) return ""
    val dirs = listOf("N", "NE", "E", "SE", "S", "SW", "W", "NW")
    return dirs[(((deg % 360) / 45).roundToInt()) % 8]
}
