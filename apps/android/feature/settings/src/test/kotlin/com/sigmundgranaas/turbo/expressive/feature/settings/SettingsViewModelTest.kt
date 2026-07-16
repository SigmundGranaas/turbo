package com.sigmundgranaas.turbo.expressive.feature.settings

import app.cash.turbine.test
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class MutableSettingsRepository : SettingsRepository {
    val state = MutableStateFlow(UserSettings())
    override val settings: Flow<UserSettings> = state
    override suspend fun setCompassOrientation(enabled: Boolean) = state.update { it.copy(compassOrientation = enabled) }
    override suspend fun setFollowLocation(enabled: Boolean) = state.update { it.copy(followLocation = enabled) }
    override suspend fun setMetricUnits(metric: Boolean) = state.update { it.copy(metricUnits = metric) }
    override suspend fun setThemeMode(mode: ThemeMode) = state.update { it.copy(themeMode = mode) }
    override suspend fun setCloudSyncEnabled(enabled: Boolean) = state.update { it.copy(cloudSyncEnabled = enabled) }
    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) = state.update { it.copy(downloadOverWifiOnly = enabled) }
    override suspend fun setBaseLayer(layer: com.sigmundgranaas.turbo.expressive.domain.BaseLayer) = state.update { it.copy(baseLayer = layer) }
    override suspend fun addCustomTileSource(source: com.sigmundgranaas.turbo.expressive.domain.CustomTileSource) =
        state.update { it.copy(customTileSources = it.customTileSources + source, selectedCustomSourceId = source.id) }
    override suspend fun removeCustomTileSource(id: String) =
        state.update { it.copy(customTileSources = it.customTileSources.filterNot { s -> s.id == id }) }
    override suspend fun selectCustomTileSource(id: String?) = state.update { it.copy(selectedCustomSourceId = id) }
    override suspend fun setLocationDotColor(colorHex: String?) = state.update { it.copy(locationDotColorHex = colorHex) }
    override suspend fun setShowHeadingBeam(enabled: Boolean) = state.update { it.copy(showHeadingBeam = enabled) }
    override suspend fun setGestures(gestures: com.sigmundgranaas.turbo.expressive.domain.GestureSettings) = state.update { it.copy(gestures = gestures) }
    override suspend fun setExperimentalTrails(enabled: Boolean) = state.update { it.copy(experimentalTrails = enabled) }
    override suspend fun setExperimentalClouds(enabled: Boolean) = state.update { it.copy(experimentalClouds = enabled) }
    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) =
        state.update { it.copy(lastCameraLat = lat, lastCameraLng = lng, lastCameraZoom = zoom) }
}

@OptIn(ExperimentalCoroutinesApi::class)


class SettingsViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    @Test
    fun `setters persist and the state reflects them`() = runTest(mainRule.dispatcher) {
        val repo = MutableSettingsRepository()
        val vm = SettingsViewModel(repo, FakeAuthRepository())

        vm.state.test {
            assertEquals(UserSettings(), awaitItem()) // defaults

            vm.setMetric(false)
            assertFalse(awaitItem().metricUnits)

            vm.setFollow(true)
            assertTrue(awaitItem().followLocation)

            cancelAndIgnoreRemainingEvents()
        }
        assertFalse(repo.state.value.metricUnits)
        assertTrue(repo.state.value.followLocation)
    }

    @Test
    fun `gesture tunables and experimental flags round-trip through settings`() = runTest(mainRule.dispatcher) {
        val repo = MutableSettingsRepository()
        val vm = SettingsViewModel(repo, FakeAuthRepository())

        // A stricter rotation gate + faster long-press — the values the map
        // detector reads, so this proves the settings actually reach it.
        vm.setGestures(
            com.sigmundgranaas.turbo.expressive.domain.GestureSettings(longPressMs = 400L, rotationGateDeg = 20f),
        )
        vm.setExperimentalTrails(true)
        mainRule.dispatcher.scheduler.advanceUntilIdle()

        assertEquals(400L, repo.state.value.gestures.longPressMs)
        assertEquals(20f, repo.state.value.gestures.rotationGateDeg, 0f)
        assertTrue(repo.state.value.experimentalTrails)
        assertFalse(repo.state.value.experimentalClouds) // untouched stays off
    }
}
