package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/** Bottom card for the measuring tool: running total distance + undo/clear/done. */
@Composable
internal fun MeasureCard(
    points: List<LatLng>,
    onUndo: () -> Unit,
    onClear: () -> Unit,
    onDone: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    val metric = LocalMetricUnits.current
    val total = GeoMetrics.pathLengthMeters(points)
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(TurboRadius.xl),
        color = cs.surfaceContainerHigh,
        shadowElevation = 4.dp,
    ) {
        Column(Modifier.padding(horizontal = 18.dp, vertical = 14.dp)) {
            Text("Measure", style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
            Text(
                if (points.size < 2) "Tap the map to add points" else Units.distance(total, metric),
                style = MaterialTheme.typography.headlineSmall,
                color = cs.onSurface,
            )
            if (points.size >= 2) {
                Text("${points.size} points", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
            Spacer(Modifier.width(0.dp))
            Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TextButton(onClick = onUndo, enabled = points.isNotEmpty()) { Text("Undo") }
                TextButton(onClick = onClear, enabled = points.isNotEmpty()) { Text("Clear") }
                Spacer(Modifier.width(1.dp))
                Button(onClick = onDone) { Text("Done") }
            }
        }
    }
}
