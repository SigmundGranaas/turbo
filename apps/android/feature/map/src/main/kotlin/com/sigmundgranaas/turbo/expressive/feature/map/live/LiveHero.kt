package com.sigmundgranaas.turbo.expressive.feature.map.live

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigmundgranaas.turbo.expressive.core.data.LiveStats
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.components.WaveStrip
import androidx.compose.ui.res.stringResource

/** Accent palette for a live surface — red while recording, green while following. */
private data class LiveAccent(val accent: Color, val onAccent: Color)

@Composable
private fun liveAccent(recording: Boolean): LiveAccent {
    val cs = MaterialTheme.colorScheme
    val dark = androidx.compose.foundation.isSystemInDarkTheme()
    return if (recording) {
        LiveAccent(Color(0xFFE0492F), if (dark) Color(0xFFFF9E92) else Color(0xFFB3271A))
    } else {
        LiveAccent(Color(0xFF2E7D32), if (dark) Color(0xFF7FD99A) else Color(0xFF1C5A26))
    }
}

/**
 * THE shared, color-drenched live hero — the single widget core the design folds
 * into both the lock-screen Live Update and this in-app sheet, so the two can't
 * diverge. One skeleton (status row · hero-number row · wavy live strip), with a
 * route-progress bar added only while following. [recording] vs following flips
 * just the accent and the numbers shown.
 */
@Composable
fun LiveHero(
    stats: LiveStats,
    metric: Boolean,
    title: String,
    modifier: Modifier = Modifier,
    radius: Int = 28,
) {
    val cs = MaterialTheme.colorScheme
    val a = liveAccent(stats.recording)
    val onContainer = cs.onPrimaryContainer

    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(radius.dp))
            .background(cs.primaryContainer)
            .padding(start = 18.dp, end = 18.dp, top = 16.dp, bottom = 18.dp),
    ) {
        // Row A — status chip + label + right meta.
        Row(verticalAlignment = Alignment.CenterVertically) {
            StatusChip(recording = stats.recording, accent = a)
            Spacer(Modifier.width(10.dp))
            Text(
                title,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700),
                color = onContainer,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).testTag("liveTitle"),
            )
            Spacer(Modifier.width(8.dp))
            val meta = if (stats.recording) {
                stringResource(if (stats.paused) R.string.live_pause else R.string.live_moving)
            } else {
                stringResource(R.string.live_route_length, Units.distance(stats.routeDistanceM ?: 0.0, metric))
            }
            Text(
                meta,
                style = MaterialTheme.typography.labelMedium,
                color = onContainer.copy(alpha = .7f),
                maxLines = 1,
            )
        }
        Spacer(Modifier.height(12.dp))

        // Row B — hero number + symmetric right block.
        val (heroNum, heroUnit) = heroNumber(stats, metric)
        val unitLabel = if (stats.recording) heroUnit else "$heroUnit ${stringResource(R.string.live_km_left)}"
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                heroNum,
                style = MaterialTheme.typography.displayMedium.copy(fontWeight = FontWeight.W800, letterSpacing = (-2).sp),
                color = onContainer,
                modifier = Modifier.testTag("liveHeroNumber"),
            )
            Spacer(Modifier.width(7.dp))
            Text(
                unitLabel,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700),
                color = onContainer.copy(alpha = .8f),
                modifier = Modifier.padding(bottom = 6.dp),
            )
            Spacer(Modifier.weight(1f))
            RightBlock(stats, metric, onContainer)
        }
        Spacer(Modifier.height(14.dp))

        // Row C — wavy live strip + current speed (both modes).
        Row(verticalAlignment = Alignment.CenterVertically) {
            WaveStrip(color = cs.primary, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(12.dp))
            Row(verticalAlignment = Alignment.Bottom) {
                Text(
                    Units.speedValue(stats.speedMps ?: 0.0, metric),
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800),
                    color = onContainer,
                )
                Spacer(Modifier.width(4.dp))
                Text(
                    Units.speedUnit(metric),
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
                    color = onContainer.copy(alpha = .7f),
                    modifier = Modifier.padding(bottom = 2.dp),
                )
            }
        }

        // Row D — route progress (following only).
        if (!stats.recording) {
            Spacer(Modifier.height(14.dp))
            FollowProgress(stats, metric, onContainer)
        }
    }
}

@Composable
private fun StatusChip(recording: Boolean, accent: LiveAccent) {
    val dark = androidx.compose.foundation.isSystemInDarkTheme()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .height(28.dp)
            .clip(CircleShape)
            .background(accent.accent.copy(alpha = if (dark) .32f else .18f))
            .padding(start = 9.dp, end = 12.dp),
    ) {
        if (recording) {
            // Blinking REC dot.
            val transition = rememberInfiniteTransition(label = "blink")
            val a = transition.animateFloat(
                initialValue = 1f, targetValue = 0.25f,
                animationSpec = infiniteRepeatable(tween(650), RepeatMode.Reverse), label = "blinkAlpha",
            ).value
            Box(Modifier.size(9.dp).alpha(a).clip(CircleShape).background(accent.accent))
        } else {
            Icon(Icons.Rounded.CheckCircle, null, tint = accent.onAccent, modifier = Modifier.size(14.dp))
        }
        Spacer(Modifier.width(7.dp))
        Text(
            stringResource(if (recording) R.string.live_rec else R.string.live_on_route),
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W800, letterSpacing = 0.5.sp),
            color = accent.onAccent,
            modifier = Modifier.testTag("liveStatus"),
        )
    }
}

@Composable
private fun RightBlock(stats: LiveStats, metric: Boolean, onContainer: Color) {
    Column(horizontalAlignment = Alignment.End, modifier = Modifier.padding(bottom = 3.dp)) {
        if (stats.recording) {
            Text(
                Units.elevation(stats.ascentM ?: 0.0, metric),
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.W800),
                color = onContainer,
            )
            Text(stringResource(R.string.live_ascent_up), style = MaterialTheme.typography.labelSmall, color = onContainer.copy(alpha = .75f))
        } else {
            Text(
                formatShortDuration(stats.etaSeconds ?: 0),
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.W800),
                color = onContainer,
            )
            Text(
                stringResource(R.string.live_arrival_label),
                style = MaterialTheme.typography.labelSmall, color = onContainer.copy(alpha = .75f),
            )
        }
    }
}

@Composable
private fun FollowProgress(stats: LiveStats, metric: Boolean, onContainer: Color) {
    val cs = MaterialTheme.colorScheme
    val fraction = (stats.fraction ?: 0.0).toFloat().coerceIn(0f, 1f)
    val total = stats.routeDistanceM ?: 0.0
    val done = total * fraction
    Column {
        Box(
            Modifier.fillMaxWidth().height(10.dp).clip(RoundedCornerShape(5.dp))
                .background(onContainer.copy(alpha = .16f)),
        ) {
            Box(Modifier.fillMaxWidth(fraction).height(10.dp).clip(RoundedCornerShape(5.dp)).background(cs.primary).testTag("liveProgressFill"))
        }
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(
                stringResource(R.string.live_progress, Units.distance(done, metric), Units.distance(total, metric)),
                style = MaterialTheme.typography.labelSmall, color = onContainer.copy(alpha = .75f),
            )
            Text("${(fraction * 100).toInt()}%", style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.W800), color = onContainer)
        }
    }
}

/** The big hero number + unit: distance covered (recording) or distance left (following). */
private fun heroNumber(stats: LiveStats, metric: Boolean): Pair<String, String> {
    val meters = if (stats.recording) stats.distanceM else (stats.distanceRemainingM ?: 0.0)
    val full = Units.distance(meters, metric)
    val number = full.substringBeforeLast(' ', full)
    val unit = if (full.contains(' ')) full.substringAfterLast(' ') else ""
    return number to if (stats.recording) unit else unit
}

internal fun formatShortDuration(seconds: Int): String {
    val totalMin = seconds / 60
    return if (totalMin >= 60) "%d h %02d".format(totalMin / 60, totalMin % 60) else "$totalMin min"
}

/** A running clock for the recording title: "MM:SS" (or "H:MM:SS" past an hour). */
internal fun formatLiveClock(seconds: Int): String {
    val h = seconds / 3600
    val m = (seconds % 3600) / 60
    val s = seconds % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
}
