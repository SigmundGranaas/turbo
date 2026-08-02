package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.ScreenLockRotation
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material.icons.rounded.Tune
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

/**
 * Settings, in five named sections and two doors.
 *
 * It used to be a single unlabelled run of twenty-odd rows in which a
 * millisecond slider for long-press timing, a free-text URL for the pack
 * host and a routing-engine override sat between "Units" and "About" —
 * all of them looking exactly as important as each other. The controls
 * have not changed; what changed is that the sections have names, and the
 * ones that exist for measuring the app rather than using it moved behind
 * [AdvancedSettingsScreen].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onOpenAbout: () -> Unit = {},
    onOpenAccount: () -> Unit = {},
    onOpenAdvanced: () -> Unit = {},
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

            SettingsSection(stringResource(R.string.settings_appearance)) {
                Text(
                    stringResource(
                        when (settings.themeMode) {
                            ThemeMode.System -> R.string.appearance_system
                            ThemeMode.Light -> R.string.appearance_light
                            ThemeMode.Dark -> R.string.appearance_dark
                        },
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(top = 10.dp),
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 8.dp, bottom = 12.dp),
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

            SettingsSection(stringResource(R.string.settings_section_map)) {
                ListRowItem(
                    Icons.Rounded.Explore, stringResource(R.string.settings_compass),
                    subtitle = stringResource(R.string.settings_compass_sub),
                    trailing = { Switch(settings.compassOrientation, { haptics.toggle(it); viewModel.setCompass(it) }) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.MyLocation, stringResource(R.string.settings_follow),
                    trailing = { Switch(settings.followLocation, { haptics.toggle(it); viewModel.setFollow(it) }) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                // Durable home for the compass long-press "Lock rotation" toggle — the
                // reliable unlock path when the compass is hidden (map pointing north).
                ListRowItem(
                    Icons.Rounded.ScreenLockRotation, stringResource(R.string.settings_lock_rotation),
                    subtitle = stringResource(R.string.settings_lock_rotation_sub),
                    trailing = { Switch(settings.rotationLocked, { haptics.toggle(it); viewModel.setRotationLocked(it) }, modifier = Modifier.testTag("rotationLockSwitch")) },
                )
                HorizontalDivider(color = cs.outlineVariant)
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

            SettingsSection(stringResource(R.string.settings_section_general)) {
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
            }

            // Both of these change what tapping "Download" actually does, so
            // they belong next to each other and in front of the user — the
            // device build especially, which turns a download into minutes
            // of work on the phone.
            SettingsSection(stringResource(R.string.settings_section_offline)) {
                ListRowItem(
                    Icons.Rounded.Wifi, stringResource(R.string.settings_wifi_only),
                    subtitle = stringResource(R.string.settings_wifi_only_sub),
                    trailing = { Switch(settings.downloadOverWifiOnly, { haptics.toggle(it); viewModel.setWifiOnly(it) }, modifier = Modifier.testTag("wifiOnlySwitch")) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.CloudDownload, stringResource(R.string.settings_routing_device_build),
                    subtitle = stringResource(R.string.settings_routing_device_build_sub),
                    trailing = {
                        Switch(
                            settings.buildPacksOnDevice,
                            { haptics.toggle(it); viewModel.setBuildPacksOnDevice(it) },
                            modifier = Modifier.testTag("buildPacksOnDevice"),
                        )
                    },
                )
            }

            // Two doors, no heading — a heading over navigation rows would
            // name a category that does not exist.
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Tune, stringResource(R.string.settings_advanced),
                    subtitle = stringResource(R.string.settings_advanced_sub),
                    trailing = { Icon(Icons.Rounded.ChevronRight, null, tint = cs.onSurfaceVariant) },
                    modifier = Modifier.clickable(onClick = onOpenAdvanced).testTag("openAdvanced"),
                )
                HorizontalDivider(color = cs.outlineVariant)
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
