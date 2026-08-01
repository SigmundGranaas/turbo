package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.TextButton
import com.sigmundgranaas.turbo.expressive.domain.RouteEngine
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveRecord
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveStats
import com.sigmundgranaas.turbo.expressive.domain.DistanceBucket
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Science
import androidx.compose.material.icons.rounded.ScreenLockRotation
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material.icons.rounded.TouchApp
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.components.rememberTurboHaptics
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onOpenAbout: () -> Unit = {},
    onOpenAccount: () -> Unit = {},
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val settings by viewModel.state.collectAsStateWithLifecycle()
    val account by viewModel.account.collectAsStateWithLifecycle()
    val haptics = rememberTurboHaptics()

    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.action_back))
                    }
                },
            )
        },
    ) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()),
        ) {
            Spacer(Modifier.height(4.dp))
            // Account header — the REAL signed-in identity (or a sign-in prompt);
            // tapping opens the account screen either way.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth()
                    .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.primaryContainer)
                    .clickable(onClick = onOpenAccount)
                    .padding(18.dp)
                    .testTag("accountHeader"),
            ) {
                val initial = account?.email?.trim()?.firstOrNull()?.uppercaseChar()?.toString() ?: "?"
                Cookie(size = 56.dp, fill = cs.surface) { Text(initial, style = MaterialTheme.typography.titleLarge, color = cs.onPrimaryContainer) }
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    if (account != null) {
                        Text(stringResource(R.string.settings_account_signed_in), style = MaterialTheme.typography.titleMedium, color = cs.onPrimaryContainer)
                        Text(account!!.email, style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer)
                    } else {
                        Text(stringResource(R.string.settings_account_sign_in), style = MaterialTheme.typography.titleMedium, color = cs.onPrimaryContainer)
                        Text(stringResource(R.string.settings_account_sign_in_sub), style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer)
                    }
                }
                Icon(Icons.Rounded.ChevronRight, null, tint = cs.onPrimaryContainer)
            }

            Spacer(Modifier.height(14.dp))
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Palette, stringResource(R.string.settings_appearance),
                    subtitle = stringResource(
                        when (settings.themeMode) {
                            ThemeMode.System -> R.string.appearance_system
                            ThemeMode.Light -> R.string.appearance_light
                            ThemeMode.Dark -> R.string.appearance_dark
                        },
                    ),
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    ThemeMode.entries.forEach { mode ->
                        val label = stringResource(
                            when (mode) {
                                ThemeMode.System -> R.string.theme_system
                                ThemeMode.Light -> R.string.theme_light
                                ThemeMode.Dark -> R.string.theme_dark
                            },
                        )
                        FilterChip(
                            selected = settings.themeMode == mode,
                            onClick = { viewModel.setThemeMode(mode) },
                            label = { Text(label) },
                            modifier = Modifier.testTag("theme_${mode.name}"),
                        )
                    }
                }
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Explore, stringResource(R.string.settings_compass), subtitle = stringResource(R.string.settings_compass_sub),
                    trailing = { Switch(settings.compassOrientation, { haptics.toggle(it); viewModel.setCompass(it) }) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.MyLocation, stringResource(R.string.settings_follow),
                    trailing = { Switch(settings.followLocation, { haptics.toggle(it); viewModel.setFollow(it) }) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Straighten, stringResource(R.string.settings_units),
                    subtitle = stringResource(if (settings.metricUnits) R.string.units_metric else R.string.units_imperial),
                    trailing = { Switch(settings.metricUnits, { haptics.toggle(it); viewModel.setMetric(it) }, modifier = Modifier.testTag("unitsSwitch")) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.CloudSync, stringResource(R.string.settings_cloud_sync),
                    subtitle = stringResource(R.string.settings_cloud_sync_sub),
                    trailing = { Switch(settings.cloudSyncEnabled, { haptics.toggle(it); viewModel.setCloudSync(it) }, modifier = Modifier.testTag("cloudSyncSwitch")) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Wifi, stringResource(R.string.settings_wifi_only),
                    subtitle = stringResource(R.string.settings_wifi_only_sub),
                    trailing = { Switch(settings.downloadOverWifiOnly, { haptics.toggle(it); viewModel.setWifiOnly(it) }, modifier = Modifier.testTag("wifiOnlySwitch")) },
                )
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.MyLocation, stringResource(R.string.settings_location_marker),
                    subtitle = stringResource(R.string.settings_location_marker_sub),
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.horizontalScroll(rememberScrollState()).padding(bottom = 12.dp),
                ) {
                    // Default (blue) = null; the rest are the shared track palette so
                    // colours read consistently across the app's pickers.
                    DotSwatch(Color(0xFF1A73E8), selected = settings.locationDotColorHex == null) {
                        haptics.toggle(true); viewModel.setLocationDotColor(null)
                    }
                    LocationDotColors.forEach { hex ->
                        DotSwatch(dotColorOf(hex), selected = settings.locationDotColorHex.equals(hex, ignoreCase = true)) {
                            haptics.toggle(true); viewModel.setLocationDotColor(hex)
                        }
                    }
                }
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Navigation, stringResource(R.string.settings_heading_beam),
                    subtitle = stringResource(R.string.settings_heading_beam_sub),
                    trailing = { Switch(settings.showHeadingBeam, { haptics.toggle(it); viewModel.setShowHeadingBeam(it) }, modifier = Modifier.testTag("headingBeamSwitch")) },
                )
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.TouchApp, stringResource(R.string.settings_gestures),
                    subtitle = stringResource(R.string.settings_gestures_sub),
                )
                val g = settings.gestures
                GestureSlider(
                    label = stringResource(R.string.settings_gesture_long_press),
                    value = g.longPressMs.toFloat(),
                    range = 250f..800f,
                    valueLabel = "${g.longPressMs} ms",
                    testTag = "gestureLongPress",
                ) { viewModel.setGestures(g.copy(longPressMs = it.toLong())) }
                GestureSlider(
                    label = stringResource(R.string.settings_gesture_rotation),
                    value = g.rotationGateDeg,
                    range = 5f..30f,
                    valueLabel = "${g.rotationGateDeg.toInt()}°",
                    testTag = "gestureRotation",
                ) { viewModel.setGestures(g.copy(rotationGateDeg = it)) }
                GestureSlider(
                    label = stringResource(R.string.settings_gesture_move_guard),
                    value = g.movementGuardDp,
                    range = 8f..32f,
                    valueLabel = "${g.movementGuardDp.toInt()} dp",
                    testTag = "gestureMoveGuard",
                ) { viewModel.setGestures(g.copy(movementGuardDp = it)) }
                GestureSlider(
                    label = stringResource(R.string.settings_gesture_flick),
                    value = g.flingHalfLifeMs.toFloat(),
                    range = 150f..600f,
                    valueLabel = "${g.flingHalfLifeMs} ms",
                    testTag = "gestureFlick",
                ) { viewModel.setGestures(g.copy(flingHalfLifeMs = it.toLong())) }
                HorizontalDivider(color = cs.outlineVariant)
                // Durable home for the compass long-press "Lock rotation" toggle — the
                // reliable unlock path when the compass is hidden (map pointing north).
                ListRowItem(
                    Icons.Rounded.ScreenLockRotation, stringResource(R.string.settings_lock_rotation),
                    subtitle = stringResource(R.string.settings_lock_rotation_sub),
                    trailing = { Switch(settings.rotationLocked, { haptics.toggle(it); viewModel.setRotationLocked(it) }, modifier = Modifier.testTag("rotationLockSwitch")) },
                )
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Science, stringResource(R.string.settings_experimental),
                    subtitle = stringResource(R.string.settings_experimental_sub),
                )
                ListRowItem(
                    Icons.Rounded.Hiking, stringResource(R.string.settings_experimental_trails),
                    trailing = { Switch(settings.experimentalTrails, { haptics.toggle(it); viewModel.setExperimentalTrails(it) }, modifier = Modifier.testTag("experimentalTrails")) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Cloud, stringResource(R.string.settings_experimental_clouds),
                    trailing = { Switch(settings.experimentalClouds, { haptics.toggle(it); viewModel.setExperimentalClouds(it) }, modifier = Modifier.testTag("experimentalClouds")) },
                )
            }
            // Routing engine + the last few solves.
            //
            // In the shipped build, not behind a debug flag, because the
            // question it answers can only be answered here: whether the
            // phone can route is a property of the RELEASE APK on real
            // silicon — R8 has run, the ABI split has happened, the .so
            // is the one that was published — and a debug build proves
            // none of it. Left in Settings, under a plain warning, at the
            // bottom, where a curious user finding it costs them a slower
            // route and nothing else.
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Route, stringResource(R.string.settings_routing),
                    subtitle = stringResource(R.string.settings_routing_sub),
                )
                RouteEnginePicker(
                    selected = settings.routeEngine,
                    onSelect = { haptics.toggle(true); viewModel.setRouteEngine(it) },
                )
                // Where packs come from. Editable because the host that
                // cuts them is the one part of the stack that can be down
                // for weeks, and the published APK is the only build the
                // on-device measurement is valid on — so it has to be
                // re-pointable without cutting a new release.
                HorizontalDivider(color = cs.outlineVariant)
                var packSource by remember(settings.packSourceUrl) {
                    mutableStateOf(settings.packSourceUrl.orEmpty())
                }
                OutlinedTextField(
                    value = packSource,
                    onValueChange = { packSource = it },
                    label = { Text(stringResource(R.string.settings_routing_pack_source)) },
                    placeholder = { Text(RoutingPack.DEFAULT_SOURCE) },
                    supportingText = { Text(stringResource(R.string.settings_routing_pack_source_hint)) },
                    singleLine = true,
                    trailingIcon = {
                        TextButton(
                            onClick = { viewModel.setPackSourceUrl(packSource) },
                            enabled = packSource != settings.packSourceUrl.orEmpty(),
                            modifier = Modifier.testTag("packSourceSave"),
                        ) { Text(stringResource(R.string.settings_routing_pack_source_save)) }
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp)
                        .testTag("packSourceField"),
                )
                ListRowItem(
                    Icons.Rounded.Info, stringResource(R.string.settings_routing_shadow),
                    subtitle = stringResource(R.string.settings_routing_shadow_sub),
                    trailing = {
                        Switch(
                            settings.routeShadowCompare,
                            { haptics.toggle(it); viewModel.setRouteShadowCompare(it) },
                            modifier = Modifier.testTag("routeShadowCompare"),
                        )
                    },
                )
                val solves by viewModel.routeSolves.collectAsStateWithLifecycle()
                if (solves.isNotEmpty()) {
                    HorizontalDivider(color = cs.outlineVariant)
                    RouteSolveSummary(RouteSolveStats.from(solves))
                    HorizontalDivider(color = cs.outlineVariant)
                    RouteSolveList(solves, onClear = viewModel::clearRouteSolves)
                }
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Info, stringResource(R.string.settings_about),
                    subtitle = stringResource(R.string.settings_about_sub),
                    trailing = { Icon(Icons.Rounded.ChevronRight, null, tint = cs.onSurfaceVariant) },
                    modifier = Modifier.clickable(onClick = onOpenAbout),
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

/** The my-position dot palette: the shared track palette, so colour pickers read
 *  the same across the app. The default blue is offered separately (= null pref). */
private val LocationDotColors = listOf(
    "#C75B39", "#2563EB", "#059669", "#7C3AED", "#DB2777", "#D97706", "#0891B2", "#475569",
)

/** "#RRGGBB" → [Color]; falls back to the default blue on malformed input. */
private fun dotColorOf(hex: String): Color {
    val h = hex.removePrefix("#")
    if (h.length != 6 || h.any { it.digitToIntOrNull(16) == null }) return Color(0xFF1A73E8)
    return Color(0xFF000000 or h.toLong(16))
}

@Composable
private fun DotSwatch(color: Color, selected: Boolean, onClick: () -> Unit) {
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

/** A labelled slider row for one gesture tunable (Settings → Gestures). The
 *  current value shows on the right so the abstract number is legible. */
@Composable
private fun GestureSlider(
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
/** Megabytes, one decimal. The pack is tens of MB; finer is noise. */
private fun formatSize(bytes: Long): String = "%.0f MB".format(bytes / 1_000_000.0)

@Composable
private fun RouteEnginePicker(
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
private fun RouteSolveList(
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

@Composable
private fun SettingsGroup(content: @Composable () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.padding(horizontal = 16.dp, vertical = 7.dp).fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .padding(horizontal = 18.dp, vertical = 4.dp),
    ) { content() }
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
private fun RouteSolveSummary(stats: RouteSolveStats) {
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
