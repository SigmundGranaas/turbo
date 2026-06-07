package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DragHandle
import androidx.compose.material.icons.rounded.Flag
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.roundToInt

private val RowHeight = 64.dp

/**
 * Manage the route's ordered waypoints: see every stop (start → vias → destination),
 * drag a stop by its handle to reorder, or remove it. Reordering commits once on
 * drop (one re-solve), not per pixel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun WaypointsSheet(
    waypoints: List<LatLng>,
    onMove: (from: Int, to: Int) -> Unit,
    onRemove: (Int) -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    val rowPx = with(density) { RowHeight.toPx() }

    // Local mirror so the list reorders live under the finger; re-keyed when the
    // upstream route changes (after a commit or a remove).
    val items: SnapshotStateList<LatLng> = remember(waypoints) { waypoints.toMutableStateList() }
    var dragIndex by remember { mutableStateOf<Int?>(null) }
    var dragStart by remember { mutableStateOf<Int?>(null) }
    var dragOffset by remember { mutableFloatStateOf(0f) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 22.dp, end = 22.dp, bottom = 28.dp)) {
            Text(
                stringResource(R.string.track_stops_title),
                style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W700),
                color = cs.onSurface,
            )
            Text(
                stringResource(R.string.track_stops_hint),
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 14.dp),
            )

            Column {
                items.forEachIndexed { index, point ->
                    val dragging = dragIndex == index
                    Surface(
                        shape = RoundedCornerShape(18.dp),
                        color = if (dragging) cs.secondaryContainer else cs.surfaceContainerHigh,
                        shadowElevation = if (dragging) 6.dp else 0.dp,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(RowHeight)
                            .padding(vertical = 4.dp)
                            .zIndex(if (dragging) 1f else 0f)
                            .graphicsLayer { translationY = if (dragging) dragOffset else 0f }
                            .testTag("wpRow_$index"),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 12.dp)) {
                            StopBadge(index = index, last = index == items.lastIndex)
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(stopLabel(index, items.lastIndex), style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                                Text(formatCoords(point), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, maxLines = 1)
                            }
                            // Remove (endpoints stay — a route needs a start + destination).
                            if (index != 0 && index != items.lastIndex) {
                                IconButton(onClick = { onRemove(index) }, modifier = Modifier.testTag("wpRemove_$index")) {
                                    Icon(Icons.Rounded.Close, stringResource(R.string.wp_remove), tint = cs.onSurfaceVariant)
                                }
                            }
                            // Drag handle — long-press then drag to reorder.
                            Icon(
                                Icons.Rounded.DragHandle,
                                stringResource(R.string.wp_reorder),
                                tint = cs.onSurfaceVariant,
                                modifier = Modifier
                                    .size(28.dp)
                                    .pointerInput(items.size) {
                                        detectDragGesturesAfterLongPress(
                                            onDragStart = {
                                                dragStart = index; dragIndex = index; dragOffset = 0f
                                            },
                                            onDragEnd = {
                                                val from = dragStart; val to = dragIndex
                                                if (from != null && to != null && from != to) onMove(from, to)
                                                dragIndex = null; dragStart = null; dragOffset = 0f
                                            },
                                            onDragCancel = { dragIndex = null; dragStart = null; dragOffset = 0f },
                                            onDrag = { change, amount ->
                                                change.consume()
                                                val cur = dragIndex ?: return@detectDragGesturesAfterLongPress
                                                dragOffset += amount.y
                                                val target = (cur + (dragOffset / rowPx).roundToInt()).coerceIn(0, items.lastIndex)
                                                if (target != cur) {
                                                    items.add(target, items.removeAt(cur))
                                                    dragOffset -= (target - cur) * rowPx
                                                    dragIndex = target
                                                }
                                            },
                                        )
                                    },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun StopBadge(index: Int, last: Boolean) {
    val cs = MaterialTheme.colorScheme
    Box(Modifier.size(34.dp).clip(CircleShape).background(cs.primary), contentAlignment = Alignment.Center) {
        when {
            index == 0 -> Icon(Icons.Rounded.TripOrigin, null, tint = cs.onPrimary, modifier = Modifier.size(18.dp))
            last -> Icon(Icons.Rounded.Flag, null, tint = cs.onPrimary, modifier = Modifier.size(18.dp))
            else -> Text("$index", style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.W700), color = cs.onPrimary)
        }
    }
}

@Composable
private fun stopLabel(index: Int, last: Int): String = when (index) {
    0 -> stringResource(R.string.wp_start)
    last -> stringResource(R.string.wp_end)
    else -> stringResource(R.string.wp_stop, index)
}
