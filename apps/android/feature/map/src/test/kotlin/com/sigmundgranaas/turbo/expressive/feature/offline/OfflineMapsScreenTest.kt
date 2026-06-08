package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class StubOfflineTileManager(initial: List<OfflineRegionInfo>) : OfflineTileManager {
    private val flow = MutableStateFlow(initial)
    override val regions: StateFlow<List<OfflineRegionInfo>> = flow
    val deleted = mutableListOf<Long>()
    override fun refresh() = Unit
    override fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) = Unit
    override fun delete(id: Long) {
        deleted += id
        flow.value = flow.value.filterNot { it.id == id }
    }
}

private val stubGeo = object : ReverseGeocodeRepository {
    override suspend fun describe(point: LatLng): Outcome<LocationDescription> = Outcome.Success(LocationDescription("Here"))
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class OfflineMapsScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `empty state explains how to download`() {
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(emptyList()), stubGeo))
        }
        composeRule.onNodeWithText("No offline maps yet").assertIsDisplayed()
    }

    @Test
    fun `a downloading region shows its name and progress`() {
        val region = OfflineRegionInfo(id = 1, name = "Tromsø", complete = false, progress = 0.42f, sizeBytes = 5_000_000)
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(listOf(region)), stubGeo))
        }
        composeRule.onNodeWithText("Tromsø").assertIsDisplayed()
        composeRule.onNodeWithText("Downloading… 42%").assertIsDisplayed()
    }

    @Test
    fun `tapping delete removes the region`() {
        val region = OfflineRegionInfo(id = 9, name = "Lofoten", complete = true, progress = 1f, sizeBytes = 12_000_000)
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo))
        }
        composeRule.onNodeWithContentDescription("Delete Lofoten").performClick()
        composeRule.waitForIdle()
        // Tapping delete only opens the confirmation — nothing removed yet.
        assertTrue(manager.deleted.isEmpty())
        composeRule.onNodeWithText("Delete").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.deleted.contains(9L))
        composeRule.onNodeWithText("No offline maps yet").assertIsDisplayed()
    }
}
