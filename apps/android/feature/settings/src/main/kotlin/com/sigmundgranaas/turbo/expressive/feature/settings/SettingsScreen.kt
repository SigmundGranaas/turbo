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
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val settings by viewModel.state.collectAsStateWithLifecycle()
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
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth()
                    .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.primaryContainer).padding(18.dp),
            ) {
                Cookie(size = 56.dp, fill = cs.surface) { Text("S", style = MaterialTheme.typography.titleLarge, color = cs.onPrimaryContainer) }
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text("Sigmund G.", style = MaterialTheme.typography.titleMedium, color = cs.onPrimaryContainer)
                    Text("sigmund@sandring.no", style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer)
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

@Composable
private fun SettingsGroup(content: @Composable () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.padding(horizontal = 16.dp, vertical = 7.dp).fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .padding(horizontal = 18.dp, vertical = 4.dp),
    ) { content() }
}
