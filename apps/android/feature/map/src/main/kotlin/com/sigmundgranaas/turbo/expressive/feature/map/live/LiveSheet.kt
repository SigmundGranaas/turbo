package com.sigmundgranaas.turbo.expressive.feature.map.live

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.LocalFireDepartment
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Schedule
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material.icons.rounded.TrendingDown
import androidx.compose.material.icons.rounded.TrendingUp
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.data.LiveStats
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.components.LiveElevationSpark
import com.sigmundgranaas.turbo.expressive.ui.components.LiveMetricTile
import com.sigmundgranaas.turbo.expressive.ui.components.MetricTone

/** The three rest positions of the live sheet, à la the Google-Maps drawer. */
enum class LiveDetent { Peek, Half, Full }

private fun LiveDetent.up() = when (this) {
    LiveDetent.Peek -> LiveDetent.Half
    LiveDetent.Half -> LiveDetent.Full
    LiveDetent.Full -> LiveDetent.Full
}

private fun LiveDetent.down() = when (this) {
    LiveDetent.Full -> LiveDetent.Half
    LiveDetent.Half -> LiveDetent.Peek
    LiveDetent.Peek -> LiveDetent.Peek
}

/**
 * The in-app "Google-Maps sheet": a draggable bottom sheet over the live map you
 * swipe UP for full track data and DOWN to collapse. Three detents — peek · half
 * · full. The drenched [LiveHero] is the constant; the bento metric tiles, the
 * elevation profile, and the controls progressively reveal as you expand. Drives
 * both recording (Pause/Finish) and following (mute / Stop following) off the one
 * [LiveStats] read-model.
 */
@Composable
fun LiveSheet(
    stats: LiveStats,
    metric: Boolean,
    title: String,
    detent: LiveDetent,
    onDetentChange: (LiveDetent) -> Unit,
    onTogglePause: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
    elevations: List<Double> = emptyList(),
    onMute: () -> Unit = {},
    nextWaypoint: Pair<String, String>? = null,
) {
    val cs = MaterialTheme.colorScheme
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val full = maxHeight * 0.92f
        val half = maxHeight * 0.56f
        val peek = 252.dp
        val target = when (detent) {
            LiveDetent.Peek -> peek
            LiveDetent.Half -> half
            LiveDetent.Full -> full
        }
        val height by animateDpAsState(target, spring(dampingRatio = 0.82f, stiffness = 380f), label = "sheetHeight")

        // Plain holder (not snapshot state): accumulating drag must not recompose per delta.
        val dragAccum = remember { floatArrayOf(0f) }
        val dragState = rememberDraggableState { delta -> dragAccum[0] += delta }

        Column(
            Modifier
                .fillMaxWidth()
                .height(height)
                .align(Alignment.BottomCenter)
                .clip(RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp))
                .background(cs.surfaceContainerLow)
                .testTag("liveSheet"),
        ) {
            // Drag handle — swipe to change detent, tap to expand a step.
            Box(
                Modifier
                    .fillMaxWidth()
                    .draggable(
                        state = dragState,
                        orientation = Orientation.Vertical,
                        onDragStopped = {
                            if (dragAccum[0] < -DRAG_SNAP_PX) onDetentChange(detent.up())
                            else if (dragAccum[0] > DRAG_SNAP_PX) onDetentChange(detent.down())
                            dragAccum[0] = 0f
                        },
                    )
                    .testTag("liveGrab"),
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    Modifier.padding(top = 10.dp, bottom = 8.dp).size(width = 38.dp, height = 4.dp)
                        .clip(CircleShape).background(cs.onSurfaceVariant.copy(alpha = .5f)),
                )
            }

            // Scrollable readout (hero + bento + elevation); the action bar is pinned below.
            Column(
                Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState())
                    .padding(horizontal = 14.dp),
            ) {
                LiveHero(stats, metric, title)
                Spacer(Modifier.height(12.dp))
                if (stats.recording) RecordingBody(stats, metric, detent, elevations)
                else FollowBody(stats, metric, detent, elevations, nextWaypoint)
                Spacer(Modifier.height(8.dp))
            }
            // Controls stay reachable at every detent — never scrolled away.
            Column(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 12.dp)) {
                if (stats.recording) {
                    if (detent == LiveDetent.Peek) PrimaryStopRow(stats.paused, onTogglePause, onStop)
                    else FullActions(stats.paused, onTogglePause, onStop)
                } else {
                    if (detent == LiveDetent.Peek) StopFollowingButton(onStop)
                    else FollowActions(onMute, onStop)
                }
            }
        }
    }
}

@Composable
private fun RecordingBody(
    stats: LiveStats,
    metric: Boolean,
    detent: LiveDetent,
    elevations: List<Double>,
) {
    if (detent == LiveDetent.Peek) {
        SwipeHint(R.string.live_swipe_stats)
        return
    }
    // Half + Full: the live-pace bento.
    TileRow {
        LiveMetricTile(Icons.Rounded.Speed, stringResource(R.string.live_speed), Units.speedValue(stats.speedMps ?: 0.0, metric), unit = Units.speedUnit(metric), tone = MetricTone.Primary, modifier = Modifier.weight(1f))
        LiveMetricTile(Icons.Rounded.Timer, stringResource(R.string.live_avg_pace), avgPace(stats, metric), tone = MetricTone.Secondary, modifier = Modifier.weight(1f))
        LiveMetricTile(Icons.Rounded.Bolt, stringResource(R.string.live_max), Units.speedValue(stats.maxSpeedMps ?: 0.0, metric), unit = Units.speedUnit(metric), tone = MetricTone.Neutral, modifier = Modifier.weight(1f))
    }
    if (detent == LiveDetent.Full) {
        Spacer(Modifier.height(10.dp))
        TileRow {
            LiveMetricTile(Icons.Rounded.TrendingUp, stringResource(R.string.live_ascent), Units.elevation(stats.ascentM ?: 0.0, metric), tone = MetricTone.Green, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.TrendingDown, stringResource(R.string.live_descent), Units.elevation(stats.descentM ?: 0.0, metric), tone = MetricTone.Red, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.LocalFireDepartment, stringResource(R.string.live_energy), "${stats.kcal}", unit = stringResource(R.string.live_kcal), tone = MetricTone.Tertiary, modifier = Modifier.weight(1f))
        }
        ElevationCard(elevations, stats, metric)
    } else {
        SwipeHint(R.string.live_swipe_more)
    }
}

@Composable
private fun FollowBody(
    stats: LiveStats,
    metric: Boolean,
    detent: LiveDetent,
    elevations: List<Double>,
    nextWaypoint: Pair<String, String>?,
) {
    if (detent == LiveDetent.Peek) {
        SwipeHint(R.string.live_swipe_stats)
        return
    }
    TileRow {
        LiveMetricTile(Icons.Rounded.TrendingUp, stringResource(R.string.live_to_climb), Units.elevation(stats.ascentRemainingM ?: 0.0, metric), tone = MetricTone.Green, modifier = Modifier.weight(1f))
        LiveMetricTile(Icons.Rounded.Speed, stringResource(R.string.live_speed), Units.speedValue(stats.speedMps ?: 0.0, metric), unit = Units.speedUnit(metric), tone = MetricTone.Primary, modifier = Modifier.weight(1f))
        LiveMetricTile(Icons.Rounded.Schedule, stringResource(R.string.live_ahead), "${(stats.etaSeconds ?: 0) / 60}", unit = stringResource(R.string.live_min), tone = MetricTone.Secondary, modifier = Modifier.weight(1f))
    }
    if (detent == LiveDetent.Full) {
        Spacer(Modifier.height(10.dp))
        TileRow {
            LiveMetricTile(Icons.Rounded.TrendingDown, stringResource(R.string.live_descent), Units.elevation(stats.descentM ?: 0.0, metric), tone = MetricTone.Red, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.Terrain, stringResource(R.string.live_altitude), Units.elevation(stats.altitudeM ?: 0.0, metric), tone = MetricTone.Neutral, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.LocalFireDepartment, stringResource(R.string.live_energy), "${stats.kcal}", unit = stringResource(R.string.live_kcal), tone = MetricTone.Tertiary, modifier = Modifier.weight(1f))
        }
        ElevationCard(elevations, stats, metric)
        if (nextWaypoint != null) {
            Spacer(Modifier.height(10.dp))
            NextWaypointRow(caption = nextWaypoint.first, name = nextWaypoint.second)
        }
    } else {
        SwipeHint(R.string.live_swipe_more)
    }
}

@Composable
private fun TileRow(content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) =
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp), content = content)

@Composable
private fun ElevationCard(elevations: List<Double>, stats: LiveStats, metric: Boolean) {
    if (elevations.size < 2) return
    Spacer(Modifier.height(10.dp))
    val value = if (stats.recording) {
        stringResource(R.string.live_elevation_now, Units.elevation(stats.altitudeM ?: elevations.last(), metric))
    } else {
        stringResource(R.string.live_elevation_to_go, Units.elevation(stats.altitudeM ?: elevations.last(), metric), Units.elevation(stats.ascentRemainingM ?: 0.0, metric))
    }
    LiveElevationSpark(
        elevations = elevations,
        progress = (stats.fraction ?: 1.0).toFloat(),
        label = stringResource(R.string.live_elevation),
        value = value,
    )
}

@Composable
private fun PrimaryStopRow(paused: Boolean, onTogglePause: () -> Unit, onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth().padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(onClick = onTogglePause, modifier = Modifier.weight(1f).height(52.dp).testTag("livePause")) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.live_resume else R.string.live_pause), fontWeight = FontWeight.W700)
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            modifier = Modifier.width(64.dp).height(52.dp).testTag("liveFinish")
                .clearAndSetSemantics { contentDescription = "Finish" },
        ) { Icon(Icons.Rounded.Stop, null, modifier = Modifier.size(22.dp)) }
    }
}

@Composable
private fun FullActions(paused: Boolean, onTogglePause: () -> Unit, onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(onClick = onTogglePause, modifier = Modifier.weight(1f).height(56.dp).testTag("livePause")) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.live_resume else R.string.live_pause), fontWeight = FontWeight.W700)
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            modifier = Modifier.weight(1f).height(56.dp).testTag("liveFinish"),
        ) {
            Icon(Icons.Rounded.Stop, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.live_finish), fontWeight = FontWeight.W700)
        }
    }
}

@Composable
private fun FollowActions(onMute: () -> Unit, onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(
            onClick = onMute,
            modifier = Modifier.size(56.dp).testTag("liveMute").clearAndSetSemantics { contentDescription = "Mute" },
            contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp),
        ) { Icon(Icons.Rounded.VolumeUp, null, modifier = Modifier.size(22.dp)) }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            modifier = Modifier.weight(1f).height(56.dp).testTag("liveStop"),
        ) {
            Icon(Icons.Rounded.Close, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(9.dp))
            Text(stringResource(R.string.live_stop_following), fontWeight = FontWeight.W700)
        }
    }
}

@Composable
private fun StopFollowingButton(onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Button(
        onClick = onStop,
        colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
        modifier = Modifier.fillMaxWidth().height(52.dp).padding(top = 2.dp).testTag("liveStop"),
    ) {
        Icon(Icons.Rounded.Close, null, modifier = Modifier.size(20.dp))
        Spacer(Modifier.width(9.dp))
        Text(stringResource(R.string.live_stop_following), fontWeight = FontWeight.W700)
    }
}

@Composable
private fun SwipeHint(resId: Int) {
    Spacer(Modifier.height(12.dp))
    Text(
        stringResource(resId),
        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth(),
    )
}

private fun avgPace(stats: LiveStats, metric: Boolean): String =
    Units.pace(stats.distanceM, stats.elapsedSec ?: 0, metric).substringBefore(' ')

private const val DRAG_SNAP_PX = 36f
