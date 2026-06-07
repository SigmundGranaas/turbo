package com.sigmundgranaas.turbo.expressive.feature.auth

import com.sigmundgranaas.turbo.expressive.core.sync.SyncController
import com.sigmundgranaas.turbo.expressive.core.sync.SyncOutcome
import com.sigmundgranaas.turbo.expressive.core.sync.SyncStatus
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SyncViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    private class FakeSyncController : SyncController {
        override val status = MutableStateFlow<SyncStatus>(SyncStatus.Idle)
        var syncs = 0
        override suspend fun syncNow(): SyncOutcome { syncs++; return SyncOutcome.Success }
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `status mirrors the controller`() = runTest(dispatcher) {
        val controller = FakeSyncController()
        val vm = SyncViewModel(controller)
        assertEquals(SyncStatus.Idle, vm.status.value)
        controller.status.value = SyncStatus.Syncing
        assertEquals(SyncStatus.Syncing, vm.status.value)
    }

    @Test
    fun `syncNow asks the controller to sync`() = runTest(dispatcher) {
        val controller = FakeSyncController()
        val vm = SyncViewModel(controller)
        vm.syncNow()
        advanceUntilIdle()
        assertEquals(1, controller.syncs)
    }
}
