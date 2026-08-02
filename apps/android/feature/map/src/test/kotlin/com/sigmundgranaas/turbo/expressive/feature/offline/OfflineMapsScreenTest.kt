package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
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
    val retried = mutableListOf<Long>()
    val paused = mutableListOf<Long>()
    override fun refresh() = Unit
    override fun download(spec: DownloadSpec) = Unit
    override fun retry(id: Long) { retried += id }
    override fun pause(id: Long) { paused += id }
    override fun resume(id: Long) = Unit
    override fun setNetworkAllowed(allowed: Boolean) = Unit
    override fun rename(id: Long, name: String) { renamed += id to name }
    override fun clearAmbientCache() = Unit
    val renamed = mutableListOf<Pair<Long, String>>()
    override fun estimate(spec: DownloadSpec) = OfflineEstimate(tiles = 0, bytes = 0)
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
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(emptyList()), stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("No offline maps yet").assertIsDisplayed()
    }

    @Test
    fun `a downloading region shows its name and progress`() {
        val region = OfflineRegionInfo(id = 1, name = "Tromsø", status = OfflineStatus.Downloading, progress = 0.42f, sizeBytes = 5_000_000)
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(listOf(region)), stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("Tromsø").assertIsDisplayed()
        composeRule.onNodeWithText("Downloading… 42%").assertIsDisplayed()
    }

    /**
     * A build is not a download and must not claim to be one. It runs
     * before the first tile, moves in jumps as whole phases land, and
     * takes minutes — a line reading "Downloading…" against a bar that
     * sits still for a minute is indistinguishable from a stall.
     */
    @Test
    fun `a region building its routing data says so, not downloading`() {
        val region = OfflineRegionInfo(id = 7, name = "Sulitjelma", status = OfflineStatus.Building, progress = 0.30f, sizeBytes = 0)
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(listOf(region)), stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("Building routing data on this phone… 30%").assertIsDisplayed()
    }

    /** Minutes of Kartverket over the user's connection has to be stoppable. */
    @Test
    fun `a building region can be paused`() {
        val region = OfflineRegionInfo(id = 8, name = "Junkerdal", status = OfflineStatus.Building, progress = 0.1f, sizeBytes = 0)
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithContentDescription("Pause Junkerdal").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.paused.contains(8L))
    }

    /**
     * A region that downloaded with gaps is Complete — the area works —
     * but it must admit to the gaps and offer the one-tap fix. Silence
     * here means the user meets the missing tiles as a blank square,
     * offline, with no idea it is fixable.
     */
    @Test
    fun `a complete region with gaps says so and can be topped up`() {
        val region = OfflineRegionInfo(
            id = 11, name = "Hasvik", status = OfflineStatus.Complete, progress = 1f, sizeBytes = 40_000_000,
            errorReason = "137 of 2133 tiles are missing",
        )
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("137 of 2133 tiles are missing", substring = true).assertIsDisplayed()
        composeRule.onNodeWithText("Fill gaps").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.retried.contains(11L))
    }

    /** The gap line is conditional — a clean region must not show it. */
    @Test
    fun `a clean complete region shows no gap note`() {
        val region = OfflineRegionInfo(id = 12, name = "Lofoten", status = OfflineStatus.Complete, progress = 1f, sizeBytes = 12_000_000)
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(StubOfflineTileManager(listOf(region)), stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithTag("incompleteNote").assertDoesNotExist()
    }

    @Test
    fun `tapping Retry on a failed download restarts it`() {
        val region = OfflineRegionInfo(
            id = 3, name = "Senja", status = OfflineStatus.Failed, progress = 0f, sizeBytes = 0,
            errorReason = "Area too large",
        )
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("Retry").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.retried.contains(3L))
    }

    @Test
    fun `delete is staged with an undo snackbar - undo restores the region`() {
        val region = OfflineRegionInfo(id = 9, name = "Lofoten", status = OfflineStatus.Complete, progress = 1f, sizeBytes = 12_000_000)
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithContentDescription("Delete Lofoten").performClick()
        composeRule.waitForIdle()
        // Hidden from the list, but nothing actually deleted while the snackbar runs.
        assertTrue(manager.deleted.isEmpty())
        composeRule.onNodeWithText("No offline maps yet").assertIsDisplayed()

        composeRule.onNodeWithText("Undo").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.deleted.isEmpty())
        composeRule.onNodeWithText("Lofoten").assertIsDisplayed()
    }

    @Test
    fun `tapping the name opens rename and forwards the new name`() {
        val region = OfflineRegionInfo(id = 4, name = "Tromsø", status = OfflineStatus.Complete, progress = 1f, sizeBytes = 1_000_000)
        val manager = StubOfflineTileManager(listOf(region))
        composeRule.setContent {
            OfflineMapsScreen(onBack = {}, viewModel = OfflineViewModel(manager, stubGeo, FakeOfflineSettings()))
        }
        composeRule.onNodeWithText("Tromsø").performClick()
        composeRule.onNode(hasSetTextAction()).performTextReplacement("Kvaløya")
        composeRule.onNodeWithText("Rename").performClick()
        composeRule.waitForIdle()
        assertTrue(manager.renamed.contains(4L to "Kvaløya"))
    }
}
