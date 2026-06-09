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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.AtmosphericPoint
import com.sigmundgranaas.turbo.expressive.domain.DailySummary
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
import com.sigmundgranaas.turbo.expressive.domain.TideForecast
import com.sigmundgranaas.turbo.expressive.domain.TideKind
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.ui.components.ErrorState
import com.sigmundgranaas.turbo.expressive.ui.components.weatherIcon
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudOff
import java.time.LocalDate
import kotlin.math.roundToInt

/** Full multi-day forecast, opened from the conditions card. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WeatherForecastSheet(
    point: LatLng,
    onDismiss: () -> Unit,
    viewModel: ConditionsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val state by viewModel.forecast.collectAsStateWithLifecycle()
    val ocean by viewModel.ocean.collectAsStateWithLifecycle()
    androidx.compose.runtime.LaunchedEffect(point) {
        viewModel.loadForecast(point)
        viewModel.loadOcean(point)
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(
            Modifier.fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = 20.dp).padding(bottom = 24.dp),
        ) {
            Text(stringResource(R.string.cond_forecast_title), style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Spacer(Modifier.height(14.dp))
            when (val s = state) {
                is ForecastUiState.Loading -> Box(Modifier.fillMaxWidth().height(200.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
                is ForecastUiState.Error -> ErrorState(
                    message = stringResource(R.string.cond_forecast_error_body),
                    onRetry = { viewModel.loadForecast(point) },
                    icon = Icons.Rounded.CloudOff,
                    modifier = Modifier.fillMaxWidth().height(200.dp),
                )
                is ForecastUiState.Content -> WeatherForecastContent(s.forecast)
            }
            // Ocean — waves / sea temp / current + tide table. Hidden inland (Empty).
            (ocean as? OceanUiState.Content)?.let {
                Spacer(Modifier.height(20.dp))
                OceanSection(it.marine, it.tides)
            }
        }
    }
}

/** Ocean section: marine tiles + tide high/low table. Only shown when data exists. */
@Composable
internal fun OceanSection(marine: MarineNow?, tides: TideForecast?) {
    val cs = MaterialTheme.colorScheme
    Text(stringResource(R.string.cond_ocean_title), style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
    Spacer(Modifier.height(10.dp))
    if (marine != null) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OceanTile(marine.waveHeightM?.let { "%.1f m".format(it) } ?: "—", stringResource(R.string.cond_waves), Modifier.weight(1f))
            OceanTile(marine.seaTemperatureC?.let { "${it.roundToInt()}°" } ?: "—", stringResource(R.string.cond_sea_temp), Modifier.weight(1f))
            OceanTile(marine.seaCurrentSpeedMs?.let { "%.1f m/s".format(it) } ?: "—", stringResource(R.string.cond_current), Modifier.weight(1f))
        }
    }
    if (tides != null && tides.hasData) {
        Spacer(Modifier.height(12.dp))
        tides.stationName?.let {
            Text(it, style = MaterialTheme.typography.labelLarge, color = cs.onSurfaceVariant)
            Spacer(Modifier.height(6.dp))
        }
        tides.extrema.take(6).forEach { e ->
            Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    stringResource(if (e.kind == TideKind.High) R.string.tide_high else R.string.tide_low),
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (e.kind == TideKind.High) cs.primary else cs.onSurfaceVariant,
                    modifier = Modifier.width(56.dp),
                )
                Text(hourLabel(e.timeIso), style = MaterialTheme.typography.bodyMedium, color = cs.onSurface)
                Spacer(Modifier.weight(1f))
                Text("${e.levelCm.roundToInt()} cm", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun OceanTile(value: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier.clip(RoundedCornerShape(TurboRadius.m)).background(cs.surfaceContainerHigh).padding(vertical = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(value, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
            Text(label, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
        }
    }
}

/** Stateless forecast body — day strip + hourly list — extracted so it's testable. */
@Composable
internal fun WeatherForecastContent(forecast: WeatherForecast) {
    var selectedDate by remember(forecast) { mutableStateOf(forecast.days.firstOrNull()?.date) }

    LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(forecast.days) { day ->
            DayChip(day, selected = day.date == selectedDate) { selectedDate = day.date }
        }
    }
    Spacer(Modifier.height(12.dp))
    val hours = forecast.points.filter { it.date == selectedDate }
    LazyColumn(Modifier.fillMaxWidth().height(320.dp)) {
        items(hours) { p -> HourRow(p) }
    }
}

@Composable
private fun DayChip(day: DailySummary, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clip(RoundedCornerShape(TurboRadius.l))
            .background(if (selected) cs.secondaryContainer else cs.surfaceContainerHigh)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 10.dp),
    ) {
        Text(weekday(day.date), style = MaterialTheme.typography.labelLarge, color = cs.onSurface)
        Spacer(Modifier.height(4.dp))
        Icon(weatherIcon(day.middaySymbol), null, tint = cs.primary, modifier = Modifier.size(22.dp))
        Spacer(Modifier.height(4.dp))
        Text(
            "${day.maxTempC?.roundToInt() ?: "–"}° / ${day.minTempC?.roundToInt() ?: "–"}°",
            style = MaterialTheme.typography.bodySmall,
            color = cs.onSurfaceVariant,
        )
    }
}

@Composable
private fun HourRow(p: AtmosphericPoint) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
    ) {
        Text(hourLabel(p.timeIso), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant, modifier = Modifier.width(52.dp))
        Icon(weatherIcon(p.symbol1h), null, tint = cs.primary, modifier = Modifier.size(22.dp))
        Spacer(Modifier.width(14.dp))
        Text(p.temperatureC?.let { "${it.roundToInt()}°" } ?: "—", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W600), color = cs.onSurface)
        Spacer(Modifier.weight(1f))
        if ((p.precipitation1hMm ?: 0.0) > 0.0) {
            Text("%.1f mm".format(p.precipitation1hMm), style = MaterialTheme.typography.bodySmall, color = cs.primary)
            Spacer(Modifier.width(12.dp))
        }
        Text(p.windSpeedMs?.let { "${it.roundToInt()} m/s" } ?: "", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
    }
}

private val WEEKDAYS = arrayOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

private fun weekday(date: String): String =
    runCatching { WEEKDAYS[LocalDate.parse(date).dayOfWeek.value - 1] }.getOrDefault(date.takeLast(5))

private fun hourLabel(timeIso: String): String = timeIso.substringAfter('T', "").take(5).ifEmpty { "—" }
