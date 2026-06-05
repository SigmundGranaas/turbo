package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
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
}
