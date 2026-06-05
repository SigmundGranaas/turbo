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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AcUnit
import androidx.compose.material.icons.rounded.Air
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.WbSunny
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.SpecRow
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard
import com.sigmundgranaas.turbo.expressive.ui.theme.DangerColors
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/** One column of the 6-hour weather strip. */
private data class HourCell(val time: String, val icon: ImageVector, val temp: String, val wind: String)

private val forecastStrip = listOf(
    HourCell("12", Icons.Rounded.WbSunny, "-2°", "4"),
    HourCell("13", Icons.Rounded.WbSunny, "-1°", "5"),
    HourCell("14", Icons.Rounded.Cloud, "-1°", "6"),
    HourCell("15", Icons.Rounded.Cloud, "-2°", "7"),
    HourCell("16", Icons.Rounded.AcUnit, "-3°", "6"),
    HourCell("17", Icons.Rounded.AcUnit, "-4°", "5"),
)

/**
 * Weather forecast sheet for a tapped point: the headline now-conditions, a
 * 6-hour strip, and a few key metrics. Sample data — wired to a real source via
 * the ConditionsSource seam later.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WeatherSheet(
    placeName: String,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, bottom = 32.dp)) {
            SectionLabel("Weather · yr.no", color = cs.primary)
            Spacer(Modifier.height(4.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.WbSunny, null, tint = cs.primary, modifier = Modifier.size(40.dp))
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Text("-2°", style = MaterialTheme.typography.displaySmall, color = cs.onSurface)
                    Text(placeName, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(16.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                forecastStrip.forEach { HourColumn(it) }
            }
            Spacer(Modifier.height(18.dp))
            TurboCard {
                SpecRow("Feels like", "-7°")
                SpecRow("Wind", "5 m/s NW, gust 11")
                SpecRow("Precip 1h", "0.2 mm")
                SpecRow("Freezing level", "600 m")
            }
        }
    }
}

@Composable
private fun HourColumn(cell: HourCell) {
    val cs = MaterialTheme.colorScheme
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(cell.time, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
        Spacer(Modifier.height(6.dp))
        Icon(cell.icon, null, tint = cs.primary, modifier = Modifier.size(24.dp))
        Spacer(Modifier.height(6.dp))
        Text(cell.temp, style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Rounded.Air, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(11.dp))
            Text(cell.wind, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
        }
    }
}

/**
 * Avalanche sheet for a tapped slope: Varsom danger level, the headline problem,
 * and a per-aspect/elevation breakdown. Sample data — wired to Varsom later.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AvalancheSheet(
    region: String,
    level: Int = 3,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val danger = DangerColors.all[(level - 1).coerceIn(0, 4)]
    val labels = listOf("Low", "Moderate", "Considerable", "High", "Very high")
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, bottom = 32.dp)) {
            SectionLabel("Avalanche · Varsom", color = cs.primary)
            Spacer(Modifier.height(8.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl))
                    .background(danger.copy(alpha = 0.14f)).padding(16.dp),
            ) {
                DangerBars(level)
                Spacer(Modifier.width(14.dp))
                Column {
                    Text("${labels[(level - 1).coerceIn(0, 4)]} · Level $level", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800), color = cs.onSurface)
                    Text(region, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(14.dp))
            TurboCard {
                SectionLabel("Avalanche problem")
                Spacer(Modifier.height(8.dp))
                Text("Wind slab", style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
                Text("Fresh wind-transported snow on N–E aspects above 900 m. Triggering likely on steep, convex rolls.", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }
            Spacer(Modifier.height(12.dp))
            TurboCard {
                SpecRow("Most exposed", "N, NE, E")
                SpecRow("Critical elevation", "> 900 m")
                SpecRow("Trend", "Stabilising tonight")
            }
        }
    }
}

/** 5-bar danger badge (green→very-dark-red), filled to [level]. */
@Composable
private fun DangerBars(level: Int) {
    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        val inactive = MaterialTheme.colorScheme.surfaceContainerHighest
        for (n in 1..5) {
            Box(
                Modifier.size(width = 9.dp, height = (8 + n * 4).dp).clip(RoundedCornerShape(3.dp))
                    .background(if (n <= level) DangerColors.all[n - 1] else inactive),
            )
        }
    }
}
