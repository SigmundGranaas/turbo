package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class FakeSettingsRepository : SettingsRepository {
    private val state = MutableStateFlow(UserSettings())
    override val settings: Flow<UserSettings> = state
    override suspend fun setCompassOrientation(enabled: Boolean) = state.update { it.copy(compassOrientation = enabled) }
    override suspend fun setFollowLocation(enabled: Boolean) = state.update { it.copy(followLocation = enabled) }
    override suspend fun setMetricUnits(metric: Boolean) = state.update { it.copy(metricUnits = metric) }
    override suspend fun setThemeMode(mode: ThemeMode) = state.update { it.copy(themeMode = mode) }
    override suspend fun setCloudSyncEnabled(enabled: Boolean) = state.update { it.copy(cloudSyncEnabled = enabled) }
    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) = state.update { it.copy(downloadOverWifiOnly = enabled) }
    override suspend fun setBaseLayer(layer: com.sigmundgranaas.turbo.expressive.domain.BaseLayer) = state.update { it.copy(baseLayer = layer) }
    override suspend fun setLocationDotColor(colorHex: String?) = state.update { it.copy(locationDotColorHex = colorHex) }
    override suspend fun setShowHeadingBeam(enabled: Boolean) = state.update { it.copy(showHeadingBeam = enabled) }
    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) =
        state.update { it.copy(lastCameraLat = lat, lastCameraLng = lng, lastCameraZoom = zoom) }
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SettingsScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `toggling the units switch flips metric to imperial`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository()))
        }
        // Default is metric.
        composeRule.onNodeWithText("Metric · km, m").assertExists()

        composeRule.onNodeWithTag("unitsSwitch").performScrollTo().performClick()

        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Imperial · mi, ft").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Imperial · mi, ft").assertExists()
    }

    @Test
    fun `selecting the Dark theme chip updates the appearance subtitle`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository()))
        }
        composeRule.onNodeWithText("Follow system").assertExists() // System default

        composeRule.onNodeWithTag("theme_Dark").performScrollTo().performClick()

        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Dark theme").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Dark theme").assertExists()
    }
}
