package com.sigmundgranaas.turbo.expressive.feature.map.route

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
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
import androidx.compose.material.icons.rounded.DragHandle
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.roundToInt

private val RowHeight = 56.dp

/** Stop-kind colours from the design (start green, end red, vias use the categorical palette). */
val StopStart = Color(0xFF2E7D32)
val StopEnd = Color(0xFFC0392B)

/**
 * Per-stop colour: start green, end red; every via takes a stable categorical colour keyed on
 * its coordinate (via [StopPalette]) so it survives a reorder or re-solve — the stop keeps its
 * colour even as its index changes.
 */
fun stopColor(index: Int, last: Int, point: LatLng): Color = when (index) {
    0 -> StopStart
    last -> StopEnd
    else -> Color(StopPalette.colorOf(point))
}

/**
 * Pure drag-to-reorder target: where a stop dragged from [from] by [dyPx] vertical pixels lands,
 * given each row is [rowHeightPx] tall. Rounds to the nearest row and clamps into the list, so a
 * drag can never target a slot that doesn't exist. No Compose → unit-tested; the drag handle in
 * [StopRow] feeds it the live delta and commits the result once on release.
 */
fun dragReorderTarget(from: Int, dyPx: Float, rowHeightPx: Float, count: Int): Int {
    if (count <= 1 || rowHeightPx <= 0f) return from
    val steps = (dyPx / rowHeightPx).roundToInt()
    return (from + steps).coerceIn(0, count - 1)
}

/**
 * Manage the route's ordered stops (the design's "Define route" editor): every stop
 * (start → vias → destination) as a row with reorder controls, an A/B/C… letter badge
 * (flag for the destination), a role label + a name/coords line, and a remove button for
 * intermediate stops. Each row lazily resolves a human name via [onResolve]; until then it
 * shows trimmed coordinates in the same fixed slot (in-place swap, no reflow).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WaypointsSheet(
    waypoints: List<LatLng>,
    statText: String,
    onMove: (from: Int, to: Int) -> Unit,
    onRemove: (Int) -> Unit,
    onAddStop: () -> Unit,
    onDismiss: () -> Unit,
    nameFor: (LatLng) -> String? = { null },
    onResolve: (LatLng) -> Unit = {},
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        WaypointsList(waypoints, statText, onMove, onRemove, onAddStop, nameFor, onResolve)
    }
}

/** Host-free stop list — the body of [WaypointsSheet], exercised directly in tests. */
@Composable
fun WaypointsList(
    waypoints: List<LatLng>,
    statText: String,
    onMove: (from: Int, to: Int) -> Unit,
    onRemove: (Int) -> Unit,
    onAddStop: () -> Unit,
    nameFor: (LatLng) -> String? = { null },
    onResolve: (LatLng) -> Unit = {},
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
                    count = waypoints.size,
                    point = point,
                    secondary = StopLabels.label(nameFor(point), point),
                    onResolve = { onResolve(point) },
                    // Two ways to reorder: the drag handle (its detector consumes the vertical
                    // drag so the modal sheet doesn't steal it), and explicit ↑/↓ as the
                    // always-there accessible fallback. Both land on the same [onMove].
                    onReorder = onMove,
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
    count: Int,
    point: LatLng,
    secondary: String,
    onResolve: () -> Unit,
    onReorder: (from: Int, to: Int) -> Unit,
    onMoveUp: (() -> Unit)?,
    onMoveDown: (() -> Unit)?,
    onRemove: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val badge = stopColor(index, last, point)
    val isEndpoint = index == 0 || index == last
    val density = LocalDensity.current
    val haptics = LocalHapticFeedback.current
    // Row pitch in px — the Column stacks rows at [RowHeight] with no spacing, so this is the
    // step the pure [dragReorderTarget] rounds against.
    val rowHeightPx = with(density) { RowHeight.toPx() }
    // Drag state: the picked-up row lifts (translationY + raised z) and commits its landing slot
    // once, on release. The list itself doesn't reshuffle mid-drag (that would re-solve every
    // frame) — it's a clean pick-up-and-drop.
    var dragging by remember { mutableStateOf(false) }
    var dragDy by remember { mutableFloatStateOf(0f) }
    // Kick off the (lazy, cached) reverse-geocode once this row is composed. Never blocks:
    // the row already shows coords; a resolved name swaps in place on the next recomposition.
    LaunchedEffect(point) { onResolve() }
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = if (dragging) cs.surfaceContainerHighest else cs.surfaceContainerHigh,
        shadowElevation = if (dragging) 8.dp else 0.dp,
        modifier = Modifier
            .fillMaxWidth()
            .height(RowHeight)
            .padding(vertical = 3.dp)
            .zIndex(if (dragging) 1f else 0f)
            .graphicsLayer { translationY = if (dragging) dragDy else 0f }
            .testTag("wpRow_$index"),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, end = 6.dp)) {
            // Drag handle — owns the vertical drag. `detectDragGestures` consumes the pointer, so
            // the drag reorders instead of dismissing the modal sheet (the reason plain rows
            // couldn't drag). Endpoints (start/destination) keep their role and don't drag.
            if (!isEndpoint) {
                Icon(
                    Icons.Rounded.DragHandle,
                    stringResource(R.string.wp_reorder),
                    tint = cs.onSurfaceVariant,
                    modifier = Modifier
                        .size(24.dp)
                        .testTag("wpDrag_$index")
                        .pointerInput(index, count, rowHeightPx) {
                            detectDragGestures(
                                onDragStart = {
                                    dragging = true
                                    dragDy = 0f
                                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                },
                                onDragCancel = { dragging = false; dragDy = 0f },
                                onDragEnd = {
                                    val target = dragReorderTarget(index, dragDy, rowHeightPx, count)
                                    dragging = false
                                    dragDy = 0f
                                    if (target != index) onReorder(index, target)
                                },
                                onDrag = { change, delta ->
                                    change.consume()
                                    dragDy += delta.y
                                },
                            )
                        },
                )
            } else {
                Spacer(Modifier.width(24.dp))
            }
            // Reorder controls: move this stop earlier / later in the order (accessible fallback).
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
                // Single fixed-height line: coords now, name later — an in-place swap, no reflow.
                Text(secondary, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, maxLines = 1, modifier = Modifier.testTag("wpSecondary_$index"))
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
