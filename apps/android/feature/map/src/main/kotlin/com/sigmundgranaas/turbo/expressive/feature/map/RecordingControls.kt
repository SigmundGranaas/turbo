package com.sigmundgranaas.turbo.expressive.feature.map

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
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.feature.recording.R as RecR
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * The recording surface for the home map: live distance / time / pace plus
 * pause and stop. Stateless — it renders the [journey] read-model and reports
 * intent through the callbacks, so it can be exercised headlessly. The same
 * journey concept backs route planning ([RouteCard]); recording just takes over
 * the bottom slot while it's active.
 */
@Composable
internal fun RecordingControls(
    journey: ActiveJourney,
    metric: Boolean,
    onPause: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    val elapsed = journey.elapsedSec ?: 0
    val live = !journey.paused
    Column(modifier.fillMaxWidth()) {
        Surface(
            shape = RoundedCornerShape(TurboRadius.xl),
            color = cs.surfaceContainerHigh,
            shadowElevation = 3.dp,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(Modifier.padding(horizontal = 20.dp, vertical = 16.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(10.dp).clip(CircleShape).background(if (live) Color(0xFFE0432B) else cs.onSurfaceVariant))
                    Spacer(Modifier.width(8.dp))
                    Text(
                        stringResource(if (live) RecR.string.rec_recording else RecR.string.rec_paused),
                        style = MaterialTheme.typography.labelLarge,
                        color = cs.onSurface,
                        modifier = Modifier.testTag("recStatus"),
                    )
                }
                Spacer(Modifier.height(12.dp))
                Row {
                    Stat(Units.distance(journey.distanceM, metric), stringResource(RecR.string.rec_distance), Modifier.weight(1f).testTag("recDistance"))
                    Stat(formatRecElapsed(elapsed), stringResource(RecR.string.rec_time), Modifier.weight(1f))
                    Stat(Units.pace(journey.distanceM, elapsed, metric), stringResource(RecR.string.rec_pace), Modifier.weight(1f))
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(
            horizontalArrangement = Arrangement.End,
            modifier = Modifier.fillMaxWidth(),
        ) {
            FloatingActionButton(
                onClick = onPause,
                containerColor = cs.secondaryContainer,
                contentColor = cs.onSecondaryContainer,
                modifier = Modifier.size(56.dp).testTag("recPause"),
            ) {
                Icon(
                    if (journey.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
                    stringResource(if (journey.paused) RecR.string.rec_resume else RecR.string.rec_pause),
                )
            }
            Spacer(Modifier.width(12.dp))
            FloatingActionButton(
                onClick = onStop,
                containerColor = cs.primary,
                contentColor = cs.onPrimary,
                modifier = Modifier.size(56.dp).testTag("recStop"),
            ) { Icon(Icons.Rounded.Stop, stringResource(RecR.string.rec_stop)) }
        }
    }
}

@Composable
private fun Stat(value: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Column(modifier) {
        Text(value, style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W800), color = cs.onSurface)
        Text(label, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
    }
}

private fun formatRecElapsed(seconds: Int): String {
    val h = seconds / 3600
    val m = (seconds % 3600) / 60
    val s = seconds % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
}
