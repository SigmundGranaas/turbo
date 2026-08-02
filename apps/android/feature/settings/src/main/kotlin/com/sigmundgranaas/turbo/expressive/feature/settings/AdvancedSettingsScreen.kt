package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveStats
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.components.rememberTurboHaptics

/**
 * The controls that are not for everyone.
 *
 * Gesture tunables in milliseconds and degrees, unfinished map layers, and
 * the routing engine override with its measurement readout — all of which
 * were sitting in the main Settings list between "Units" and "About",
 * indistinguishable from the settings a hiker actually sets. They are
 * still in the shipped build, still one tap from Settings, and still not
 * behind a debug flag: whether the phone can route is a property of the
 * RELEASE APK on real silicon, and a debug build proves none of it. They
 * are simply no longer the first thing you scroll past.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AdvancedSettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val settings by viewModel.state.collectAsStateWithLifecycle()
    val haptics = rememberTurboHaptics()

    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        stringResource(R.string.settings_advanced),
                        style = MaterialTheme.typography.headlineSmall,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.action_back))
                    }
                },
            )
        },
    ) { inner ->
        Column(Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState())) {
            SettingsSection(stringResource(R.string.settings_gestures)) {
                Text(
                    stringResource(R.string.settings_gestures_sub),
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
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
            }

            SettingsSection(stringResource(R.string.settings_experimental)) {
                Text(
                    stringResource(R.string.settings_experimental_sub),
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
                )
                ListRowItem(
                    Icons.Rounded.Hiking, stringResource(R.string.settings_experimental_trails),
                    trailing = {
                        Switch(
                            settings.experimentalTrails,
                            { haptics.toggle(it); viewModel.setExperimentalTrails(it) },
                            modifier = Modifier.testTag("experimentalTrails"),
                        )
                    },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Cloud, stringResource(R.string.settings_experimental_clouds),
                    trailing = {
                        Switch(
                            settings.experimentalClouds,
                            { haptics.toggle(it); viewModel.setExperimentalClouds(it) },
                            modifier = Modifier.testTag("experimentalClouds"),
                        )
                    },
                )
            }

            SettingsSection(stringResource(R.string.settings_routing)) {
                Text(
                    stringResource(R.string.settings_routing_sub),
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
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
                        .padding(vertical = 8.dp)
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
            Spacer(Modifier.height(24.dp))
        }
    }
}
