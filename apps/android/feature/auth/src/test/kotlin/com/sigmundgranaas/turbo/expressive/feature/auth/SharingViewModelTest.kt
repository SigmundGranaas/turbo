package com.sigmundgranaas.turbo.expressive.feature.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.sync.LinkRedemption
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SharingViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    private class FakeSharingRepository(private val code: Outcome<String>) : SharingRepository {
        override suspend fun friendCode(): Outcome<String> = code
        override suspend fun createLink(resourceId: String, role: String) = Outcome.Success("https://x/link/t")
        override suspend fun redeemLink(token: String) = Outcome.Success(LinkRedemption("r", "path", "viewer"))
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `loads the friend code on init`() = runTest(dispatcher) {
        val vm = SharingViewModel(FakeSharingRepository(Outcome.Success("turbo-AB12CD")))
        advanceUntilIdle()
        assertEquals("turbo-AB12CD", vm.friendCode)
    }

    @Test
    fun `friend code stays null when the lookup fails`() = runTest(dispatcher) {
        val vm = SharingViewModel(FakeSharingRepository(Outcome.Failure(RuntimeException("offline"))))
        advanceUntilIdle()
        assertNull(vm.friendCode)
    }
}
