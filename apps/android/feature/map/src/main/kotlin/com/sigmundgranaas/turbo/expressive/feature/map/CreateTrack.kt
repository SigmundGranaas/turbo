package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.BorderStroke
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteSweep
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material.icons.rounded.EditRoad
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Gesture
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Timeline
import androidx.compose.material.icons.rounded.TouchApp
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Undo
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.roundToInt

/**
 * The three ways to build a line on the map, unified into one "Create track" tool.
 * [Route] snaps a natural route to trails (the route generator), [Line] joins
 * tapped points with straight segments, [Draw] is a freehand finger stroke. The
 * old separate route-planner and measuring tools collapse into these.
 */
enum class TrackMode(val icon: ImageVector, val labelRes: Int) {
    Route(Icons.Rounded.Route, R.string.track_mode_route),
    Line(Icons.Rounded.Timeline, R.string.track_mode_line),
    Draw(Icons.Rounded.Gesture, R.string.track_mode_draw),
}

/** Off-trail surface colour for the route mix bar (the others come from the scheme). */
private val OffTrailColor = Color(0xFFD98A2B)

/**
 * The unified bottom tool panel. Stateless: it renders the current [mode] + stats
 * and reports intent through callbacks, so it can be exercised headlessly. Mirrors
 * the design's TrackPanel — mode toggle, hero stat, route-only surface mix +
 * route-style row, and the undo/clear · Save/Follow action bar.
 */
@Composable
internal fun CreateTrackPanel(
    mode: TrackMode,
    onMode: (TrackMode) -> Unit,
    distanceText: String,
    unit: String,
    metaText: String,
    surfaces: Map<String, Double>,
    presetLabel: String,
    onRouteStyle: () -> Unit,
    stopCount: Int = 0,
    onManageStops: () -> Unit = {},
    canUndo: Boolean,
    canSave: Boolean,
    onUndo: () -> Unit,
    onClear: () -> Unit,
    onSave: () -> Unit,
    onFollow: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    val isRoute = mode == TrackMode.Route
    Surface(
        modifier = modifier.fillMaxWidth().testTag("createTrackPanel"),
        shape = RoundedCornerShape(32.dp),
        color = cs.surfaceContainerLowest,
        border = BorderStroke(1.dp, cs.outlineVariant.copy(alpha = 0.6f)),
        shadowElevation = 8.dp,
    ) {
        Column {
            Box(
                Modifier.fillMaxWidth().padding(top = 12.dp),
                contentAlignment = Alignment.TopCenter,
            ) {
                Box(Modifier.width(34.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(cs.outlineVariant))
            }

            // Zone 1 — the mode toggle (the primary decision): a connected button group
            // whose selected pill morphs its corner radius, springy M3-Expressive style.
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                TrackMode.entries.forEach { m ->
                    ModeButton(m = m, selected = m == mode, onClick = { onMode(m) }, modifier = Modifier.weight(1f))
                }
            }

            // Zone 2 — hero stat.
            Column(Modifier.padding(start = 22.dp, end = 22.dp, top = 18.dp)) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(
                        distanceText,
                        style = MaterialTheme.typography.displaySmall.copy(
                            fontSize = 40.sp,
                            lineHeight = 40.sp,
                            fontWeight = FontWeight.W800,
                            letterSpacing = (-1.6).sp,
                        ),
                        color = cs.onSurface,
                        modifier = Modifier.testTag("trackDistance"),
                    )
                    if (unit.isNotEmpty()) {
                        Spacer(Modifier.width(8.dp))
                        Text(unit, style = MaterialTheme.typography.titleMedium, color = cs.onSurfaceVariant, modifier = Modifier.padding(bottom = 3.dp))
                    }
                }
                Text(metaText, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
            }

            // Zone 3 — surface mix (route only).
            if (isRoute && surfaces.values.sum() > 0.0) {
                SurfaceMix(surfaces, Modifier.padding(start = 22.dp, end = 22.dp, top = 14.dp))
            }

            // Zone 4 — route-style selector + stops manager (route only).
            if (isRoute) {
                Surface(
                    onClick = onRouteStyle,
                    shape = RoundedCornerShape(18.dp),
                    color = cs.surfaceContainerHigh,
                    modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp).height(56.dp).testTag("routeStyleRow"),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 16.dp, end = 10.dp)) {
                        Icon(Icons.Rounded.Tune, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                        Spacer(Modifier.width(14.dp))
                        Text(stringResource(R.string.track_route_style), style = MaterialTheme.typography.titleMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                        Text(presetLabel, style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700), color = cs.primary)
                        Icon(Icons.Rounded.ExpandMore, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                    }
                }
                // Stops manager — visible once a route exists (start + destination).
                if (stopCount >= 2) {
                    Surface(
                        onClick = onManageStops,
                        shape = RoundedCornerShape(18.dp),
                        color = cs.surfaceContainerHigh,
                        modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 8.dp).height(56.dp).testTag("stopsRow"),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 16.dp, end = 10.dp)) {
                            Icon(Icons.Rounded.Flag, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                            Spacer(Modifier.width(14.dp))
                            Text(stringResource(R.string.track_stops_row), style = MaterialTheme.typography.titleMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                            Text("$stopCount", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700), color = cs.primary)
                            Icon(Icons.Rounded.ChevronRight, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                        }
                    }
                }
            }

            // Zone 5 — action bar.
            Row(
                Modifier.fillMaxWidth().padding(start = 14.dp, end = 14.dp, top = 16.dp, bottom = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                IconButton(onClick = onUndo, enabled = canUndo, modifier = Modifier.testTag("trackUndo")) {
                    Icon(Icons.Rounded.Undo, stringResource(R.string.measure_undo), tint = cs.onSurfaceVariant)
                }
                IconButton(onClick = onClear, enabled = canSave, modifier = Modifier.testTag("trackClear")) {
                    Icon(Icons.Rounded.DeleteSweep, stringResource(R.string.measure_clear), tint = cs.onSurfaceVariant)
                }
                Spacer(Modifier.weight(1f))
                FilledTonalButton(onClick = onSave, enabled = canSave, modifier = Modifier.height(48.dp).testTag("trackSave")) {
                    Text(stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_save))
                }
                Button(
                    onClick = onFollow,
                    enabled = canSave,
                    modifier = Modifier.height(48.dp).testTag("trackFollow"),
                    colors = ButtonDefaults.buttonColors(),
                ) {
                    Icon(Icons.Rounded.Navigation, null, modifier = Modifier.size(19.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.route_follow))
                }
            }
        }
    }
}

@Composable
private fun ModeButton(m: TrackMode, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    // The selected button morphs to a pill; unselected stays a soft square — the
    // Expressive "connected button group" shape transition.
    val radius by animateDpAsState(if (selected) 27.dp else 14.dp, spring(), label = "modeRadius")
    val bg by animateColorAsState(if (selected) cs.primary else cs.surfaceContainerHigh, label = "modeBg")
    val fg by animateColorAsState(if (selected) cs.onPrimary else cs.onSurfaceVariant, label = "modeFg")
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(radius),
        color = bg,
        modifier = modifier.height(54.dp).testTag("trackMode_${m.name}"),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center, modifier = Modifier.fillMaxWidth()) {
            Icon(m.icon, null, tint = fg, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(
                stringResource(m.labelRes),
                style = MaterialTheme.typography.labelLarge.copy(fontWeight = if (selected) FontWeight.W700 else FontWeight.W600),
                color = fg,
            )
        }
    }
}

/** Proportional trail/road/off-trail bar + legend (route mode), from the plan's per-surface metres. */
@Composable
private fun SurfaceMix(surfaces: Map<String, Double>, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    val total = surfaces.values.sum().takeIf { it > 0.0 } ?: return
    fun color(name: String) = when (name.lowercase()) {
        "trail" -> cs.primary
        "road" -> cs.outline
        else -> OffTrailColor
    }
    val ordered = surfaces.entries.sortedByDescending { it.value }
    Column(modifier) {
        Row(Modifier.fillMaxWidth().height(10.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            ordered.forEach { (name, meters) ->
                val frac = (meters / total).toFloat()
                if (frac > 0f) Box(Modifier.weight(frac).height(10.dp).clip(RoundedCornerShape(5.dp)).background(color(name)))
            }
        }
        Text(
            ordered.joinToString("   ·   ") { (name, meters) ->
                "${((meters / total) * 100).roundToInt()}% ${name.lowercase()}"
            },
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant,
            modifier = Modifier.padding(top = 10.dp),
        )
    }
}

/** The single dialog: route-style preset picker (Balanced / Avoid roads / …). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RouteStyleSheet(selected: RoutePreset, onSelect: (RoutePreset) -> Unit, onDismiss: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 22.dp, end = 22.dp, bottom = 28.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.Tune, null, tint = cs.primary, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(10.dp))
                Text(stringResource(R.string.track_route_style), style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W700), color = cs.onSurface)
            }
            Text(
                stringResource(R.string.track_route_style_subtitle),
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp),
            )
            RoutePreset.entries.forEach { p ->
                val sel = p == selected
                Surface(
                    onClick = { onSelect(p) },
                    shape = RoundedCornerShape(20.dp),
                    color = if (sel) cs.secondaryContainer else cs.surfaceContainerHigh,
                    modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp).testTag("preset_${p.key}"),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(14.dp)) {
                        Box(
                            Modifier.size(44.dp).clip(CircleShape).background(if (sel) cs.primary else cs.surface),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(p.icon, null, tint = if (sel) cs.onPrimary else cs.onSurfaceVariant, modifier = Modifier.size(22.dp))
                        }
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f)) {
                            Text(p.label, style = MaterialTheme.typography.titleMedium, color = if (sel) cs.onSecondaryContainer else cs.onSurface)
                            Text(p.description, style = MaterialTheme.typography.bodySmall, color = if (sel) cs.onSecondaryContainer.copy(alpha = 0.85f) else cs.onSurfaceVariant)
                        }
                        if (sel) {
                            Spacer(Modifier.width(10.dp))
                            Icon(Icons.Rounded.CheckCircle, null, tint = cs.primary, modifier = Modifier.size(24.dp))
                        }
                    }
                }
            }
        }
    }
}

/** Top-left close affordance for the active tool. */
@Composable
internal fun CreateTrackCloseButton(onClose: () -> Unit, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(
        onClick = onClose,
        shape = RoundedCornerShape(16.dp),
        color = cs.surfaceContainerHigh,
        shadowElevation = 4.dp,
        modifier = modifier.size(48.dp).testTag("trackClose"),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(Icons.Rounded.Close, stringResource(R.string.track_close), tint = cs.primary, modifier = Modifier.size(24.dp))
        }
    }
}

/** Centred title chip — "Create track". */
@Composable
internal fun CreateTrackTitleChip(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(shape = CircleShape, color = cs.surfaceContainerHigh, shadowElevation = 4.dp, modifier = modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 18.dp, vertical = 11.dp)) {
            Icon(Icons.Rounded.EditRoad, null, tint = cs.primary, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.track_title), style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700), color = cs.onSurface)
        }
    }
}

/**
 * A trimmed map control kept available while the tool owns the screen — zoom and
 * recenter, so building a route doesn't trap the camera (the full rail is hidden).
 */
@Composable
internal fun CreateTrackMapControls(
    following: Boolean,
    onLocate: () -> Unit,
    onZoomIn: () -> Unit,
    onZoomOut: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    androidx.compose.foundation.layout.Column(
        modifier = modifier,
        horizontalAlignment = Alignment.End,
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Surface(
            onClick = onLocate,
            shape = RoundedCornerShape(18.dp),
            color = if (following) cs.tertiaryContainer else cs.surfaceContainerHigh,
            shadowElevation = 3.dp,
            modifier = Modifier.size(52.dp),
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    if (following) androidx.compose.material.icons.Icons.Rounded.MyLocation else androidx.compose.material.icons.Icons.Rounded.NearMe,
                    stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_my_location),
                    tint = if (following) cs.onTertiaryContainer else cs.primary,
                    modifier = Modifier.size(24.dp),
                )
            }
        }
        Surface(shape = CircleShape, color = cs.surfaceContainerHigh, shadowElevation = 3.dp) {
            androidx.compose.foundation.layout.Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.padding(vertical = 4.dp),
            ) {
                com.sigmundgranaas.turbo.expressive.ui.components.IconBtn(
                    androidx.compose.material.icons.Icons.Rounded.Add,
                    stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_zoom_in),
                    size = 42.dp,
                    onClick = onZoomIn,
                )
                androidx.compose.material3.HorizontalDivider(modifier = Modifier.width(28.dp), color = cs.outlineVariant)
                com.sigmundgranaas.turbo.expressive.ui.components.IconBtn(
                    androidx.compose.material.icons.Icons.Rounded.Remove,
                    stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_zoom_out),
                    size = 42.dp,
                    onClick = onZoomOut,
                )
            }
        }
    }
}

/** Coachmark teaching the current mode's gesture. */
@Composable
internal fun CreateTrackHint(mode: TrackMode, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    val hint = when (mode) {
        TrackMode.Route -> R.string.track_hint_route
        TrackMode.Line -> R.string.track_hint_line
        TrackMode.Draw -> R.string.track_hint_draw
    }
    Surface(shape = CircleShape, color = cs.inverseSurface, shadowElevation = 2.dp, modifier = modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp)) {
            Icon(
                if (mode == TrackMode.Draw) Icons.Rounded.Gesture else Icons.Rounded.TouchApp,
                null,
                tint = cs.inverseOnSurface,
                modifier = Modifier.size(16.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(stringResource(hint), style = MaterialTheme.typography.labelMedium, color = cs.inverseOnSurface)
        }
    }
}

/** Coachmark shown while a planned route re-solves after a stop edit (graceful re-route). */
@Composable
internal fun CreateTrackUpdatingChip(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(shape = CircleShape, color = cs.inverseSurface, shadowElevation = 2.dp, modifier = modifier.testTag("updatingRoute")) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp)) {
            CircularProgressIndicator(modifier = Modifier.size(14.dp), strokeWidth = 2.dp, color = cs.inverseOnSurface)
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.track_updating), style = MaterialTheme.typography.labelMedium, color = cs.inverseOnSurface)
        }
    }
}
