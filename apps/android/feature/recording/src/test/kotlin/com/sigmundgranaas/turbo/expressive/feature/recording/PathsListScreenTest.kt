package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.sync.LinkRedemption
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class StubPathRepository(paths: List<SavedPath>) : PathRepository {
    private val state = MutableStateFlow(paths)
    override fun observeAll(): Flow<List<SavedPath>> = state
    override suspend fun byId(id: String): SavedPath? = state.value.firstOrNull { it.id == id }
    override suspend fun save(path: SavedPath) {}
    override suspend fun delete(id: String) {}
    override suspend fun remoteId(id: String): String? = null
}

private object NoopSharingRepository : SharingRepository {
    override suspend fun friendCode() = Outcome.Failure(RuntimeException())
    override suspend fun createLink(resourceId: String, role: String) = Outcome.Success("https://x/link/t")
    override suspend fun redeemLink(token: String) = Outcome.Success(LinkRedemption("r", "path", "viewer"))
    override suspend fun sharedResources(since: String?) = Outcome.Success(com.sigmundgranaas.turbo.expressive.core.sync.ResourceSyncPageDto())
}

private fun pathsViewModel(repo: PathRepository) = PathsViewModel(repo, NoopSharingRepository)

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class PathsListScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val track = SavedPath(
        id = "p-1",
        name = "Storsteinen Loop",
        path = GeoPath(
            points = listOf(LatLng(69.0, 18.0), LatLng(69.01, 18.01)),
            source = GeoPathSource.Recording,
            distanceM = 6100.0,
            movingTimeSeconds = 3600,
        ),
    )

    @Test
    fun `empty repository shows the empty hint`() {
        composeRule.setContent {
            PathsListScreen(onBack = {}, onOpen = {}, viewModel = pathsViewModel(StubPathRepository(emptyList())))
        }
        composeRule.onNodeWithText("No saved tracks yet").assertIsDisplayed()
    }

    @Test
    fun `saved track is listed and tapping it opens by id`() {
        var opened: String? = null
        composeRule.setContent {
            PathsListScreen(onBack = {}, onOpen = { opened = it }, viewModel = pathsViewModel(StubPathRepository(listOf(track))))
        }
        composeRule.onNodeWithText("Storsteinen Loop").assertIsDisplayed()
        composeRule.onNodeWithText("Storsteinen Loop").performClick()
        assertEquals("p-1", opened)
    }
}
