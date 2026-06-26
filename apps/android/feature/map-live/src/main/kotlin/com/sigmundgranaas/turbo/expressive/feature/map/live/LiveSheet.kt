package com.sigmundgranaas.turbo.expressive.feature.map.live

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.AnchoredDraggableDefaults
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.animateTo
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DirectionsWalk
import androidx.compose.material.icons.rounded.LocalFireDepartment
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RadioButtonUnchecked
import androidx.compose.material.icons.rounded.Schedule
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material.icons.rounded.TrendingDown
import androidx.compose.material.icons.rounded.TrendingUp
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.tracking.LiveStats
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.ui.components.LiveElevationSpark
import com.sigmundgranaas.turbo.expressive.ui.components.LiveMetricTile
import com.sigmundgranaas.turbo.expressive.ui.components.MetricTone
import com.sigmundgranaas.turbo.expressive.ui.components.pressScale

/**
 * The rest positions of the live sheet, à la the Google-Maps drawer. [Mini] is a
 * one-line status bar (title + hero number + the stop control) for when you want the
 * map almost entirely clear; Peek/Half/Full progressively reveal the full readout.
 */
enum class LiveDetent { Mini, Peek, Half, Full }

private fun LiveDetent.up() = when (this) {
    LiveDetent.Mini -> LiveDetent.Peek
    LiveDetent.Peek -> LiveDetent.Half
    LiveDetent.Half -> LiveDetent.Full
    LiveDetent.Full -> LiveDetent.Full
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
    nextWaypoint: Pair<String, String>? = null,
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val full = maxHeight * 0.92f
        val half = maxHeight * 0.56f
        // Peek must clear the hero + pinned actions (incl. the nav-bar inset) so the
        // glance state is never clipped.
        val peek = minOf(340.dp, maxHeight * 0.64f)
        // Mini: just the handle + the one-line status bar + the stop control.
        val mini = minOf(208.dp, peek)
        fun heightFor(d: LiveDetent) = when (d) {
            LiveDetent.Mini -> mini
            LiveDetent.Peek -> peek
            LiveDetent.Half -> half
            LiveDetent.Full -> full
        }

        // anchoredDraggable so the sheet FOLLOWS the finger — its height tracks the live
        // drag offset and settles to the nearest detent on release with the expressive
        // spring, instead of the old release-only jump. Anchors are top-edge offsets
        // (containerPx − detentHeightPx); Mini is the largest offset (smallest sheet).
        val containerPx = with(density) { maxHeight.toPx() }
        val anchors = remember(containerPx, mini, peek, half, full) {
            with(density) {
                DraggableAnchors {
                    LiveDetent.Mini at containerPx - mini.toPx()
                    LiveDetent.Peek at containerPx - peek.toPx()
                    LiveDetent.Half at containerPx - half.toPx()
                    LiveDetent.Full at containerPx - full.toPx()
                }
            }
        }
        val dragState = remember { AnchoredDraggableState(initialValue = detent) }
        LaunchedEffect(anchors) { dragState.updateAnchors(anchors, dragState.targetValue) }

        // Bridge the hoisted detent contract both ways. Out→in: a tap-to-step or external
        // detent change animates the sheet to that anchor. In→out: report the settled
        // detent upward once a drag/fling comes to rest.
        val settleSpec = MaterialTheme.motionScheme.defaultSpatialSpec<Float>()
        LaunchedEffect(detent) {
            if (!dragState.offset.isNaN() && dragState.targetValue != detent) {
                dragState.animateTo(detent, settleSpec)
            }
        }
        LaunchedEffect(dragState.settledValue) {
            if (dragState.settledValue != detent) onDetentChange(dragState.settledValue)
        }

        // Height = container minus the live top-edge offset; fall back to the static
        // target until anchors resolve (offset is NaN on the very first frame).
        val offsetPx = if (dragState.offset.isNaN()) {
            with(density) { containerPx - heightFor(detent).toPx() }
        } else {
            dragState.offset
        }
        val height = with(density) { (containerPx - offsetPx).toDp() }

        // Drag the whole header (handle + hero) — a big, finger-friendly target.
        val headerDrag = Modifier.anchoredDraggable(
            state = dragState,
            orientation = Orientation.Vertical,
            flingBehavior = AnchoredDraggableDefaults.flingBehavior(
                state = dragState,
                animationSpec = settleSpec,
            ),
        )
        // …and let the scrollable readout drag the sheet too (US-8) via the standard nested-
        // scroll bridge: an up-swipe grows the sheet before its content scrolls; a down-swipe at
        // the top collapses it; on release we settle to the nearest detent. Safe-additive — the
        // header drag and normal content scrolling are untouched. (Note: Compose drag gestures
        // can't be driven by `adb input swipe`, so this needs a real-device glance.)
        val sheetNestedScroll = remember(dragState, settleSpec) {
            object : NestedScrollConnection {
                override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                    val delta = available.y
                    return if (delta < 0f) Offset(0f, dragState.dispatchRawDelta(delta)) else Offset.Zero
                }

                override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset =
                    Offset(0f, dragState.dispatchRawDelta(available.y))

                override suspend fun onPostFling(consumed: Velocity, available: Velocity): Velocity {
                    // Commit the swipe to the nearest detent (raw deltas alone spring back).
                    dragState.animateTo(dragState.targetValue, settleSpec)
                    return available
                }
            }
        }
        val expandLabel = stringResource(R.string.live_expand)

        Column(
            Modifier
                .fillMaxWidth()
                .height(height)
                .align(Alignment.BottomCenter)
                .clip(RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp))
                .background(cs.surfaceContainerLow)
                .testTag("liveSheet"),
        ) {
            // Header: grab handle + hero. The whole block drags to resize; tapping the
            // handle steps the sheet open (and collapses one step from full).
            Column(headerDrag) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(40.dp)
                        .clickable(onClickLabel = expandLabel) {
                            onDetentChange(if (detent == LiveDetent.Full) LiveDetent.Half else detent.up())
                        }
                        .semantics { contentDescription = expandLabel }
                        .testTag("liveGrab"),
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier.size(width = 38.dp, height = 4.dp)
                            .clip(CircleShape).background(cs.onSurfaceVariant.copy(alpha = .5f)),
                    )
                }
                if (detent == LiveDetent.Mini) {
                    LiveMiniBar(stats, metric, title)
                } else {
                    Box(Modifier.padding(horizontal = 14.dp)) { LiveHero(stats, metric, title) }
                }
            }

            // Scrollable readout (bento + elevation); the action bar is pinned below. The nested-
            // scroll bridge lets a swipe here resize the sheet too (US-8), not just the header.
            Column(
                Modifier.fillMaxWidth().weight(1f)
                    .nestedScroll(sheetNestedScroll)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 14.dp),
            ) {
                Spacer(Modifier.height(12.dp))
                if (stats.recording) RecordingBody(stats, metric, detent, elevations)
                else FollowBody(stats, metric, detent, elevations, nextWaypoint)
                Spacer(Modifier.height(8.dp))
            }
            // Controls stay reachable at every detent — never scrolled away, and lifted
            // clear of the gesture-nav bar so they're not half-behind the system inset.
            Column(
                Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 14.dp, vertical = 12.dp),
            ) {
                // Proactively catch "I forgot to unpause and kept walking" (US-4): a banner the
                // moment buffered movement is meaningful, with a one-tap Resume.
                if (stats.showResumeNudge) {
                    ResumeNudgeBanner(stats.bufferedDistanceM, metric, onResume = onTogglePause)
                    Spacer(Modifier.height(10.dp))
                }
                if (stats.recording) {
                    if (detent == LiveDetent.Peek || detent == LiveDetent.Mini) PrimaryStopRow(stats.paused, onTogglePause, onStop)
                    else FullActions(stats.paused, onTogglePause, onStop)
                } else {
                    // Follow = Record: the same Pause/Resume + Stop pair, so a follow can be
                    // paused (and the paused walk buffered) exactly like a recording (US-4).
                    FollowActions(stats.paused, onTogglePause, onStop)
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
    if (detent == LiveDetent.Mini) return
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
    if (detent == LiveDetent.Mini) return
    if (detent == LiveDetent.Peek) {
        SwipeHint(R.string.live_swipe_stats)
        return
    }
    TileRow {
        LiveMetricTile(Icons.Rounded.TrendingUp, stringResource(R.string.live_to_climb), Units.elevation(stats.ascentRemainingM ?: 0.0, metric), tone = MetricTone.Green, modifier = Modifier.weight(1f))
        LiveMetricTile(Icons.Rounded.Speed, stringResource(R.string.live_speed), Units.speedValue(stats.speedMps ?: 0.0, metric), unit = Units.speedUnit(metric), tone = MetricTone.Primary, modifier = Modifier.weight(1f))
        val (etaValue, etaUnit) = etaValueUnit(stats.etaSeconds ?: 0)
        LiveMetricTile(Icons.Rounded.Schedule, stringResource(R.string.live_ahead), etaValue, unit = etaUnit, tone = MetricTone.Secondary, modifier = Modifier.weight(1f))
    }
    if (detent == LiveDetent.Full) {
        Spacer(Modifier.height(10.dp))
        TileRow {
            LiveMetricTile(Icons.Rounded.TrendingDown, stringResource(R.string.live_descent), Units.elevation(stats.descentM ?: 0.0, metric), tone = MetricTone.Red, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.Terrain, stringResource(R.string.live_altitude), Units.elevation(stats.altitudeM ?: 0.0, metric), tone = MetricTone.Neutral, modifier = Modifier.weight(1f))
            LiveMetricTile(Icons.Rounded.LocalFireDepartment, stringResource(R.string.live_energy), "${stats.kcal}", unit = stringResource(R.string.live_kcal), tone = MetricTone.Tertiary, modifier = Modifier.weight(1f))
        }
        ElevationCard(elevations, stats, metric)
        if (stats.phaseSplits.isNotEmpty() || stats.nextPhaseName != null) {
            Spacer(Modifier.height(10.dp))
            CheckpointsCard(stats, metric)
        } else if (nextWaypoint != null) {
            Spacer(Modifier.height(10.dp))
            NextWaypointRow(caption = nextWaypoint.first, name = nextWaypoint.second)
        }
    } else {
        SwipeHint(R.string.live_swipe_more)
    }
}

/** Split-times like a running watch (US-3): crossed checkpoints + their splits, then the next. */
@Composable
private fun CheckpointsCard(stats: LiveStats, metric: Boolean) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(cs.surfaceContainerHighest)
            .padding(horizontal = 14.dp, vertical = 12.dp).testTag("liveCheckpoints"),
    ) {
        Text(
            stringResource(R.string.live_checkpoints),
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W800),
            color = cs.onSurfaceVariant,
        )
        stats.phaseSplits.forEach { split ->
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.CheckCircle, null, tint = cs.primary, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(split.name, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.W700), color = cs.onSurface, modifier = Modifier.weight(1f))
                Text(
                    "${Units.distance(split.splitDistanceM, metric)} · ${formatLiveClock(split.splitSeconds)}",
                    style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant,
                )
            }
        }
        stats.nextPhaseName?.let { name ->
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.RadioButtonUnchecked, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.live_next_checkpoint, name), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant, modifier = Modifier.weight(1f))
                Text(Units.distance(stats.nextPhaseDistanceM ?: 0.0, metric), style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700), color = cs.onSurface)
            }
        }
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
    val pauseIs = remember { MutableInteractionSource() }
    val stopIs = remember { MutableInteractionSource() }
    Row(Modifier.fillMaxWidth().padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(
            onClick = onTogglePause,
            interactionSource = pauseIs,
            modifier = Modifier.weight(1f).height(52.dp).pressScale(pauseIs).testTag("livePause"),
        ) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.live_resume else R.string.live_pause), fontWeight = FontWeight.W700)
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            interactionSource = stopIs,
            modifier = Modifier.width(64.dp).height(52.dp).pressScale(stopIs).testTag("liveFinish")
                .clearAndSetSemantics { contentDescription = "Finish" },
        ) { Icon(Icons.Rounded.Stop, null, modifier = Modifier.size(22.dp)) }
    }
}

/** "Still moving while paused?" — the proactive resume nudge (US-4). */
@Composable
private fun ResumeNudgeBanner(bufferedM: Double, metric: Boolean, onResume: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp))
            .background(cs.tertiaryContainer).padding(start = 14.dp, end = 8.dp, top = 8.dp, bottom = 8.dp)
            .testTag("resumeNudge"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Rounded.DirectionsWalk, null, tint = cs.onTertiaryContainer, modifier = Modifier.size(20.dp))
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(
                stringResource(R.string.live_nudge_title),
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700),
                color = cs.onTertiaryContainer,
            )
            Text(
                stringResource(R.string.live_nudge_body, Units.distance(bufferedM, metric)),
                style = MaterialTheme.typography.labelSmall,
                color = cs.onTertiaryContainer.copy(alpha = .8f),
            )
        }
        FilledTonalButton(onClick = onResume, modifier = Modifier.testTag("resumeNudgeAction")) {
            Text(stringResource(R.string.live_resume), fontWeight = FontWeight.W700)
        }
    }
}

@Composable
private fun FullActions(paused: Boolean, onTogglePause: () -> Unit, onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val pauseIs = remember { MutableInteractionSource() }
    val stopIs = remember { MutableInteractionSource() }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(
            onClick = onTogglePause,
            interactionSource = pauseIs,
            modifier = Modifier.weight(1f).height(56.dp).pressScale(pauseIs).testTag("livePause"),
        ) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.live_resume else R.string.live_pause), fontWeight = FontWeight.W700)
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            interactionSource = stopIs,
            modifier = Modifier.weight(1f).height(56.dp).pressScale(stopIs).testTag("liveFinish"),
        ) {
            Icon(Icons.Rounded.Stop, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.live_finish), fontWeight = FontWeight.W700)
        }
    }
}

/** Follow controls: Pause/Resume (buffers the paused walk, US-4) beside Stop following. */
@Composable
private fun FollowActions(paused: Boolean, onTogglePause: () -> Unit, onStop: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val pauseIs = remember { MutableInteractionSource() }
    val stopIs = remember { MutableInteractionSource() }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        FilledTonalButton(
            onClick = onTogglePause,
            interactionSource = pauseIs,
            modifier = Modifier.weight(1f).height(56.dp).pressScale(pauseIs).testTag("livePause"),
        ) {
            Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(if (paused) R.string.live_resume else R.string.live_pause), fontWeight = FontWeight.W700)
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = Color.White),
            interactionSource = stopIs,
            modifier = Modifier.weight(1f).height(56.dp).pressScale(stopIs).testTag("liveStop"),
        ) {
            Icon(Icons.Rounded.Close, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(9.dp))
            Text(stringResource(R.string.live_stop_following), fontWeight = FontWeight.W700)
        }
    }
}

/** The collapsed one-liner: accent dot · title · primary number (distance / ETA). */
@Composable
private fun LiveMiniBar(stats: LiveStats, metric: Boolean, title: String) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(9.dp).clip(CircleShape).background(if (stats.recording) cs.error else cs.primary))
        Spacer(Modifier.width(10.dp))
        Text(
            title,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700),
            color = cs.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).testTag("liveTitle"),
        )
        Spacer(Modifier.width(10.dp))
        val readout = if (stats.recording) {
            Units.distance(stats.distanceM, metric)
        } else {
            val (v, u) = etaValueUnit(stats.etaSeconds ?: 0)
            "$v $u"
        }
        Text(
            readout,
            style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800),
            color = cs.primary,
            maxLines = 1,
        )
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

/** ETA as a tile value+unit: "42"/"min" under an hour, "1:20"/"h" beyond — so it
 *  never reads as an unwieldy "140 min" the way raw minutes would. */
private fun etaValueUnit(seconds: Int): Pair<String, String> {
    val totalMin = seconds / 60
    return if (totalMin >= 60) "%d:%02d".format(totalMin / 60, totalMin % 60) to "h" else "$totalMin" to "min"
}
