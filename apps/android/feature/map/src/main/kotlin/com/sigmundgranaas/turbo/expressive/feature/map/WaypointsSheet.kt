package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.TripOrigin
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.LatLng

private val RowHeight = 56.dp

/** Stop-kind colours from the design (start green, end red, vias use the scheme primary). */
internal val StopStart = Color(0xFF2E7D32)
internal val StopEnd = Color(0xFFC0392B)

/** Per-stop colour: start green, end red, otherwise the route primary. */
@Composable
internal fun stopColor(index: Int, last: Int): Color = when (index) {
    0 -> StopStart
    last -> StopEnd
    else -> MaterialTheme.colorScheme.primary
}

/**
 * Manage the route's ordered stops (the design's "Define route" editor): every stop
 * (start → vias → destination) as a row with a drag handle, an A/B/C… letter badge
 * (flag for the destination), a role label + coordinates, and a remove button for
 * intermediate stops. Drag a row by its handle to reorder (one re-solve on drop),
 * or tap "Add stop" to drop the next one on the map.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun WaypointsSheet(
    waypoints: List<LatLng>,
    statText: String,
    onMove: (from: Int, to: Int) -> Unit,
    onRemove: (Int) -> Unit,
    onAddStop: () -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        WaypointsList(waypoints, statText, onMove, onRemove, onAddStop)
    }
}

/** Host-free stop list — the body of [WaypointsSheet], exercised directly in tests. */
@Composable
internal fun WaypointsList(
    waypoints: List<LatLng>,
    statText: String,
    onMove: (from: Int, to: Int) -> Unit,
    onRemove: (Int) -> Unit,
    onAddStop: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val last = waypoints.lastIndex

    Column(Modifier.padding(start = 18.dp, end = 18.dp, bottom = 28.dp).testTag("waypointsSheet")) {
        Text(
            stringResource(R.string.track_stops_title),
            style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W700),
            color = cs.onSurface,
        )
        // Stat line: distance · ascent · time — mirrors the builder's readout.
        if (statText.isNotBlank()) {
            Text(
                statText,
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp).testTag("stopsStat"),
            )
        }
        Spacer(Modifier.height(12.dp))

        Column {
            waypoints.forEachIndexed { index, point ->
                StopRow(
                    index = index,
                    last = last,
                    coords = formatCoords(point),
                    // Reorder by explicit ↑/↓ — a drag-to-reorder inside the modal sheet
                    // loses the vertical gesture to the sheet itself, so it never fired.
                    onMoveUp = if (index > 0) ({ onMove(index, index - 1) }) else null,
                    onMoveDown = if (index < last) ({ onMove(index, index + 1) }) else null,
                    onRemove = { onRemove(index) },
                )
            }

            // "Add stop" — drops the sheet so the next map tap places a stop.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(14.dp))
                    .clickable(onClick = onAddStop)
                    .padding(horizontal = 8.dp, vertical = 8.dp)
                    .testTag("addStop"),
            ) {
                Spacer(Modifier.width(44.dp)) // align under the reorder controls
                Box(
                    Modifier.size(28.dp).clip(CircleShape).background(cs.surfaceContainerHighest),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Rounded.Add, null, tint = cs.primary, modifier = Modifier.size(18.dp)) }
                Spacer(Modifier.width(12.dp))
                Text(
                    stringResource(R.string.wp_add_stop),
                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700),
                    color = cs.primary,
                )
            }
        }
    }
}

@Composable
private fun StopRow(
    index: Int,
    last: Int,
    coords: String,
    onMoveUp: (() -> Unit)?,
    onMoveDown: (() -> Unit)?,
    onRemove: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val badge = stopColor(index, last)
    val isEndpoint = index == 0 || index == last
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = cs.surfaceContainerHigh,
        modifier = Modifier
            .fillMaxWidth()
            .height(RowHeight)
            .padding(vertical = 3.dp)
            .testTag("wpRow_$index"),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, end = 6.dp)) {
            // Reorder controls: move this stop earlier / later in the order.
            ReorderButton(Icons.Rounded.KeyboardArrowUp, R.string.wp_move_up, onMoveUp, "wpUp_$index")
            ReorderButton(Icons.Rounded.KeyboardArrowDown, R.string.wp_move_down, onMoveDown, "wpDown_$index")
            Spacer(Modifier.width(4.dp))
            Box(Modifier.size(28.dp).clip(CircleShape).background(badge), contentAlignment = Alignment.Center) {
                when {
                    index == 0 -> Icon(Icons.Rounded.TripOrigin, null, tint = Color.White, modifier = Modifier.size(15.dp))
                    index == last -> Icon(Icons.Rounded.Flag, null, tint = Color.White, modifier = Modifier.size(15.dp))
                    else -> Text(
                        ('A' + index).toString(),
                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W800),
                        color = Color.White,
                    )
                }
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(stopLabel(index, last), style = MaterialTheme.typography.titleSmall, color = cs.onSurface, maxLines = 1)
                Text(coords, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, maxLines = 1)
            }
            // Remove — vias only; a route keeps its start and destination.
            if (!isEndpoint) {
                IconButton(onClick = onRemove, modifier = Modifier.testTag("wpRemove_$index")) {
                    Icon(Icons.Rounded.Close, stringResource(R.string.wp_remove), tint = cs.onSurfaceVariant, modifier = Modifier.size(18.dp))
                }
            }
        }
    }
}

/** A compact ↑/↓ reorder control; greyed + inert at the ends ([onClick] null). */
@Composable
private fun ReorderButton(icon: androidx.compose.ui.graphics.vector.ImageVector, labelRes: Int, onClick: (() -> Unit)?, tag: String) {
    val cs = MaterialTheme.colorScheme
    IconButton(onClick = { onClick?.invoke() }, enabled = onClick != null, modifier = Modifier.size(28.dp).testTag(tag)) {
        Icon(
            icon,
            stringResource(labelRes),
            tint = if (onClick != null) cs.onSurfaceVariant else cs.onSurfaceVariant.copy(alpha = 0.3f),
            modifier = Modifier.size(20.dp),
        )
    }
}

@Composable
private fun stopLabel(index: Int, last: Int): String = when (index) {
    0 -> stringResource(R.string.wp_start)
    last -> stringResource(R.string.wp_end)
    else -> stringResource(R.string.wp_stop, index)
}
