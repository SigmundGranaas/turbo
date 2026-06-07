package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.sync.LinkRedemption
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PathsShareTest {

    private val dispatcher = StandardTestDispatcher()

    private class FakePathRepo(private val remote: String?) : PathRepository {
        override fun observeAll(): Flow<List<SavedPath>> = MutableStateFlow(emptyList())
        override suspend fun byId(id: String): SavedPath? = null
        override suspend fun save(path: SavedPath) {}
        override suspend fun delete(id: String) {}
        override suspend fun remoteId(id: String): String? = remote
    }

    private class FakeSharing(private val link: Outcome<String>) : SharingRepository {
        override suspend fun friendCode() = Outcome.Success("turbo-x")
        override suspend fun createLink(resourceId: String, role: String) = link
        override suspend fun redeemLink(token: String) = Outcome.Success(LinkRedemption("r", "path", "viewer"))
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `a synced track yields a shareable link`() = runTest(dispatcher) {
        val vm = PathsViewModel(FakePathRepo(remote = "srv-1"), FakeSharing(Outcome.Success("https://kart.sandring.no/link/tok")))
        var result: PathsViewModel.ShareLinkResult? = null
        vm.createShareLink("p-1") { result = it }
        advanceUntilIdle()
        assertTrue(result is PathsViewModel.ShareLinkResult.Ready)
        assertEquals("https://kart.sandring.no/link/tok", (result as PathsViewModel.ShareLinkResult.Ready).url)
    }

    @Test
    fun `an un-synced track reports NotSynced without calling the server`() = runTest(dispatcher) {
        val vm = PathsViewModel(FakePathRepo(remote = null), FakeSharing(Outcome.Failure(RuntimeException("should not be called"))))
        var result: PathsViewModel.ShareLinkResult? = null
        vm.createShareLink("p-1") { result = it }
        advanceUntilIdle()
        assertEquals(PathsViewModel.ShareLinkResult.NotSynced, result)
    }
}
