package com.sigmundgranaas.turbo.expressive.feature.settings

import app.cash.turbine.test
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
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
}

@OptIn(ExperimentalCoroutinesApi::class)
class SettingsViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    @Test
    fun `setters persist and the state reflects them`() = runTest(mainRule.dispatcher) {
        val repo = MutableSettingsRepository()
        val vm = SettingsViewModel(repo)

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
}
