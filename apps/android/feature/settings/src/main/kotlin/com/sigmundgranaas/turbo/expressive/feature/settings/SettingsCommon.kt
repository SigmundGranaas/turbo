package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.Box
import com.sigmundgranaas.turbo.expressive.domain.DistanceBucket
import com.sigmundgranaas.turbo.expressive.domain.RouteEngine
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveRecord
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveStats
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * A titled group of settings.
 *
 * The title is the point. This screen used to be untitled cards where the
 * first row of a card doubled as its heading — a [ListRowItem]-shaped
 * "Gestures" sitting directly above four sliders, looking exactly like a
 * setting you could tap. Nothing distinguished a label from a control,
 * so the whole screen read as one undifferentiated list of switches.
 */
@Composable
internal fun SettingsSection(
    title: String,
    content: @Composable () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    Text(
        title,
        style = MaterialTheme.typography.titleSmall,
        color = cs.primary,
        modifier = Modifier.padding(start = 32.dp, top = 14.dp, bottom = 2.dp),
    )
    SettingsGroup(content)
}

/** An untitled card. For navigation rows, where a heading would be noise. */
@Composable
internal fun SettingsGroup(content: @Composable () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.padding(horizontal = 16.dp, vertical = 7.dp).fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .padding(horizontal = 18.dp, vertical = 4.dp),
    ) { content() }
}

/** The my-position dot palette: the shared track palette, so colour pickers read
 *  the same across the app. The default blue is offered separately (= null pref). */
internal val LocationDotColors = listOf(
    "#C75B39", "#2563EB", "#059669", "#7C3AED", "#DB2777", "#D97706", "#0891B2", "#475569",
)

/** "#RRGGBB" → [Color]; falls back to the default blue on malformed input. */
internal fun dotColorOf(hex: String): Color {
    val h = hex.removePrefix("#")
    if (h.length != 6 || h.any { it.digitToIntOrNull(16) == null }) return Color(0xFF1A73E8)
    return Color(0xFF000000 or h.toLong(16))
}

@Composable
internal fun DotSwatch(color: Color, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        Modifier
            .size(34.dp)
            .clip(CircleShape)
            .background(color)
            .then(if (selected) Modifier.border(3.dp, cs.outline, CircleShape) else Modifier)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) Icon(Icons.Rounded.Check, null, tint = Color.White, modifier = Modifier.size(18.dp))
    }
}

/** A labelled slider row for one gesture tunable. The current value shows on
 *  the right so the abstract number is legible. */
@Composable
internal fun GestureSlider(
    label: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    valueLabel: String,
    testTag: String,
    onChange: (Float) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.padding(vertical = 4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
            Text(valueLabel, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
        }
        androidx.compose.material3.Slider(
            value = value.coerceIn(range.start, range.endInclusive),
            onValueChange = onChange,
            valueRange = range,
            modifier = Modifier.testTag(testTag),
        )
    }
}

/**
 * Which engine answers, as three exclusive choices.
 *
 * A segmented row rather than a switch because the third state is not
 * "off": forcing the SERVER is how a tester gets a control measurement
 * to compare a device time against, and a two-state control could not
 * express it.
 */
@Composable
internal fun RouteEnginePicker(
    selected: RouteEngine,
    onSelect: (RouteEngine) -> Unit,
) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        RouteEngine.entries.forEachIndexed { index, engine ->
            SegmentedButton(
                selected = selected == engine,
                onClick = { onSelect(engine) },
                shape = SegmentedButtonDefaults.itemShape(index, RouteEngine.entries.size),
                modifier = Modifier.testTag("routeEngine_${engine.name}"),
            ) {
                Text(
                    stringResource(
                        when (engine) {
                            RouteEngine.Auto -> R.string.settings_routing_auto
                            RouteEngine.Device -> R.string.settings_routing_device
                            RouteEngine.Server -> R.string.settings_routing_server
                        },
                    ),
                )
            }
        }
    }
}

/**
 * The last few solves, newest first.
 *
 * Dense on purpose — this is a readout to copy down, not a dashboard.
 * Engine, wall time, span and outcome are exactly the columns M1 needs
 * and nothing else is shown, because every extra field is one more
 * thing to keep true.
 */
@Composable
internal fun RouteSolveList(
    solves: List<RouteSolveRecord>,
    onClear: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(vertical = 4.dp).testTag("routeSolves")) {
        solves.forEach { r ->
            Row(
                Modifier.fillMaxWidth().padding(vertical = 3.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    r.engine.name.lowercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.primary,
                    modifier = Modifier.width(56.dp),
                )
                Text(
                    "%,d ms".format(r.durationMs),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.width(76.dp),
                )
                Text(
                    "%.1f km · %d".format(r.spanKm, r.waypoints),
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    when (r.outcome) {
                        RouteSolveRecord.Outcome.Ok -> "ok"
                        RouteSolveRecord.Outcome.NoRoute -> "no route"
                        RouteSolveRecord.Outcome.Failed -> "failed"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = if (r.outcome == RouteSolveRecord.Outcome.Failed) cs.error else cs.onSurfaceVariant,
                )
            }
            // The message is the whole value of a failed row: on a
            // release APK this is where a stripped .so or a JNA
            // reflection failure actually becomes readable.
            if (r.outcome == RouteSolveRecord.Outcome.Failed && r.detail != null) {
                Text(
                    r.detail!!,
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.error,
                    modifier = Modifier.padding(start = 56.dp, bottom = 4.dp),
                )
            }
        }
        TextButton(onClick = onClear, modifier = Modifier.testTag("clearRouteSolves")) {
            Text(stringResource(R.string.settings_routing_clear))
        }
    }
}

/**
 * The aggregate, above the raw rows.
 *
 * The list below it is the evidence; this is the conclusion. Both are
 * shown because a rate over twenty solves is easy to misread — 50 %
 * fallback sounds alarming until you see it is one solve out of two —
 * so every rate carries its denominator rather than just a percentage.
 */
@Composable
internal fun RouteSolveSummary(stats: RouteSolveStats) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp).testTag("routeSolveSummary")) {
        DistanceBucket.entries.forEach { bucket ->
            val d = stats.devicePercentiles[bucket]
            val s = stats.serverPercentiles[bucket]
            if (d == null && s == null) return@forEach
            Row(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
                Text(
                    bucket.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.width(72.dp),
                )
                Text(
                    d?.let { "phone p95 %,d ms (n=%d)".format(it.p95Ms, it.n) } ?: "phone —",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    s?.let { "server %,d (n=%d)".format(it.p95Ms, it.n) } ?: "server —",
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                )
            }
        }
        Text(
            "fallback %d/%d · coverage misses %.0f%% · failures %.0f%%".format(
                (stats.fallbackRate * stats.fallbackEligible).toInt(),
                stats.fallbackEligible,
                stats.coverageMissRate * 100,
                stats.failureRate * 100,
            ),
            style = MaterialTheme.typography.bodySmall,
            color = cs.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp),
        )
        if (stats.divergences.isNotEmpty()) {
            Text(
                "divergence worst %.0f m · %d over %.0f m (n=%d)".format(
                    stats.worstDivergenceM,
                    stats.significantDivergences,
                    com.sigmundgranaas.turbo.expressive.domain.RouteDivergence.SIGNIFICANT_M,
                    stats.divergences.size,
                ),
                style = MaterialTheme.typography.bodySmall,
                color = if (stats.significantDivergences > 0) cs.error else cs.onSurfaceVariant,
            )
        }
    }
}
