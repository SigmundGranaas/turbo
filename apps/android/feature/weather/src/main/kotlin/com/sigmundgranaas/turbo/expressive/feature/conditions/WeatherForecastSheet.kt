package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import com.sigmundgranaas.turbo.expressive.ui.components.WindArrow
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
    LaunchedEffect(point) {
        viewModel.loadForecast(point)
        viewModel.loadOcean(point)
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        ProvideWeatherImageLoader {
            Column(
                // Fixed sheet height so switching to a longer day's hour list scrolls
                // inside instead of resizing the whole sheet (the current day has fewer
                // remaining hours than full days).
                Modifier.fillMaxWidth()
                    .fillMaxHeight(0.9f)
                    .verticalScroll(rememberScrollState())
                    .navigationBarsPadding()
                    .padding(horizontal = 20.dp).padding(top = 4.dp, bottom = 24.dp),
            ) {
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
                    is ForecastUiState.Content -> {
                        // Summary of the current weather on top, then the shared day strip; the
                        // small Weather / Ocean tabs sit UNDER the days and just switch which
                        // detail the selected day shows (Ocean tab only when marine data exists).
                        val forecast = s.forecast
                        var selectedDate by remember(forecast) { mutableStateOf(forecast.days.firstOrNull()?.date) }
                        var tab by remember { mutableStateOf(WeatherTab.Weather) }
                        val oceanContent = ocean as? OceanUiState.Content

                        WeatherNowHeader(forecast.points.firstOrNull())
                        Spacer(Modifier.height(16.dp))
                        DayStrip(forecast.days, selectedDate) { selectedDate = it }
                        Spacer(Modifier.height(12.dp))
                        if (oceanContent != null) {
                            WeatherOceanTabs(tab) { tab = it }
                            Spacer(Modifier.height(12.dp))
                        }
                        if (oceanContent == null || tab == WeatherTab.Weather) {
                            HourlyList(forecast, selectedDate)
                        } else {
                            OceanSection(oceanContent.marine, oceanContent.tides, selectedDate)
                        }
                    }
                }
            }
        }
    }
}

private enum class WeatherTab { Weather, Ocean }

/** Compact Weather / Ocean segmented pills (small — they switch the detail below). */
@Composable
private fun WeatherOceanTabs(selected: WeatherTab, onSelect: (WeatherTab) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TabPill(stringResource(R.string.cond_tab_weather), selected == WeatherTab.Weather) { onSelect(WeatherTab.Weather) }
        TabPill(stringResource(R.string.cond_ocean_title), selected == WeatherTab.Ocean) { onSelect(WeatherTab.Ocean) }
    }
}

@Composable
private fun TabPill(label: String, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        Modifier
            .clip(RoundedCornerShape(50))
            .background(if (selected) cs.primary else cs.surfaceContainerHigh)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.W600),
            color = if (selected) cs.onPrimary else cs.onSurfaceVariant,
        )
    }
}

/** Current conditions up top: symbol + temperature on the left, wind/precip on the right. */
@Composable
private fun WeatherNowHeader(now: AtmosphericPoint?) {
    if (now == null) return
    val cs = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        WeatherSymbol(now.symbol1h, size = 52.dp)
        Spacer(Modifier.width(14.dp))
        Text(
            now.temperatureC?.let { "${it.roundToInt()}°" } ?: "—",
            style = MaterialTheme.typography.headlineLarge.copy(fontWeight = FontWeight.W700),
            color = cs.onSurface,
        )
        Spacer(Modifier.weight(1f))
        Column(horizontalAlignment = Alignment.End) {
            now.windSpeedMs?.let { wind ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    WindArrow(now.windFromDeg, modifier = Modifier.size(16.dp))
                    Spacer(Modifier.width(5.dp))
                    Text("${wind.roundToInt()} m/s", style = MaterialTheme.typography.bodyLarge, color = cs.onSurfaceVariant)
                }
            }
            if ((now.precipitation1hMm ?: 0.0) > 0.0) {
                Spacer(Modifier.height(2.dp))
                Text("%.1f mm".format(now.precipitation1hMm), style = MaterialTheme.typography.bodyMedium, color = cs.primary)
            }
        }
    }
}

/** Ocean view for the selected day: marine tiles + that day's tide high/low card. */
@Composable
internal fun OceanSection(marine: MarineNow?, tides: TideForecast?, selectedDate: String?) {
    val cs = MaterialTheme.colorScheme
    if (marine != null) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OceanTile(marine.waveHeightM?.let { "%.1f m".format(it) } ?: "—", stringResource(R.string.cond_waves), Modifier.weight(1f))
            OceanTile(marine.seaTemperatureC?.let { "${it.roundToInt()}°" } ?: "—", stringResource(R.string.cond_sea_temp), Modifier.weight(1f))
            OceanTile(marine.seaCurrentSpeedMs?.let { "%.1f m/s".format(it) } ?: "—", stringResource(R.string.cond_current), Modifier.weight(1f))
        }
    }
    if (tides != null && tides.hasData) {
        // Show the selected day's tide extrema (fall back to all if none match the day).
        val dayExtrema = tides.extrema
            .filter { selectedDate == null || it.timeIso.substringBefore('T') == selectedDate }
            .ifEmpty { tides.extrema }
        Spacer(Modifier.height(12.dp))
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)).background(cs.surfaceContainerHigh).padding(14.dp),
        ) {
            tides.stationName?.let {
                Text(it, style = MaterialTheme.typography.labelLarge, color = cs.onSurfaceVariant)
                Spacer(Modifier.height(8.dp))
            }
            dayExtrema.take(6).forEach { e ->
                Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
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

/** Test-facing wrapper: day strip + hourly list with its own selected-day state. */
@Composable
internal fun WeatherForecastContent(forecast: WeatherForecast) {
    var selectedDate by remember(forecast) { mutableStateOf(forecast.days.firstOrNull()?.date) }
    DayStrip(forecast.days, selectedDate) { selectedDate = it }
    Spacer(Modifier.height(8.dp))
    HourlyList(forecast, selectedDate)
}

/** The shared horizontal day picker — used by both the Weather and Ocean views. */
@Composable
private fun DayStrip(days: List<DailySummary>, selectedDate: String?, onSelect: (String) -> Unit) {
    LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(days) { day ->
            DayChip(day, selected = day.date == selectedDate) { onSelect(day.date) }
        }
    }
}

/** Hourly rows for the selected day, hairline dividers between (the sheet scrolls). */
@Composable
private fun HourlyList(forecast: WeatherForecast, selectedDate: String?) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth()) {
        forecast.points.filter { it.date == selectedDate }.forEachIndexed { i, p ->
            if (i > 0) HorizontalDivider(color = cs.outlineVariant.copy(alpha = 0.4f))
            HourRow(p)
        }
    }
}

@Composable
private fun DayChip(day: DailySummary, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val fg = if (selected) cs.onPrimary else cs.onSurface
    Box(
        modifier = Modifier
            .width(84.dp)
            .height(116.dp)
            .clip(RoundedCornerShape(50))
            .background(if (selected) cs.primary else cs.surfaceContainerHigh)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(weekday(day.date), style = MaterialTheme.typography.bodyMedium, color = fg)
            Spacer(Modifier.height(6.dp))
            WeatherSymbol(day.middaySymbol, size = 30.dp)
            Spacer(Modifier.height(6.dp))
            Text(
                "${day.maxTempC?.roundToInt() ?: "–"}° / ${day.minTempC?.roundToInt() ?: "–"}°",
                style = MaterialTheme.typography.bodySmall,
                color = if (selected) fg else cs.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun HourRow(p: AtmosphericPoint) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().padding(vertical = 10.dp),
    ) {
        Text(hourLabel(p.timeIso), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant, modifier = Modifier.width(52.dp))
        WeatherSymbol(p.symbol1h, size = 28.dp)
        Spacer(Modifier.width(14.dp))
        Text(p.temperatureC?.let { "${it.roundToInt()}°" } ?: "—", style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
        Spacer(Modifier.weight(1f))
        if ((p.precipitation1hMm ?: 0.0) > 0.0) {
            Text("%.1f mm".format(p.precipitation1hMm), style = MaterialTheme.typography.bodySmall, color = cs.primary)
            Spacer(Modifier.width(12.dp))
        }
        p.windSpeedMs?.let { wind ->
            WindArrow(p.windFromDeg, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(4.dp))
            Text("${wind.roundToInt()} m/s", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
        }
    }
}

private val WEEKDAYS = arrayOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

private fun weekday(date: String): String =
    runCatching {
        val d = LocalDate.parse(date)
        "${WEEKDAYS[d.dayOfWeek.value - 1]} ${d.dayOfMonth}"
    }.getOrDefault(date.takeLast(5))

private fun hourLabel(timeIso: String): String = timeIso.substringAfter('T', "").take(5).ifEmpty { "—" }
