package com.sigmundgranaas.turbo.expressive.feature.auth

import com.sigmundgranaas.turbo.expressive.core.sync.SyncController
import com.sigmundgranaas.turbo.expressive.core.sync.SyncOutcome
import com.sigmundgranaas.turbo.expressive.core.sync.SyncStatus
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SyncViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    /** Models a real sync: status is Syncing while it runs, Idle once it finishes.
     *  The [gate] lets a test hold the sync "in progress" to observe that state. */
    private class FakeSyncController : SyncController {
        override val status = MutableStateFlow<SyncStatus>(SyncStatus.Idle)
        val gate = CompletableDeferred<Unit>()
        override suspend fun syncNow(): SyncOutcome {
            status.value = SyncStatus.Syncing
            gate.await()
            status.value = SyncStatus.Idle
            return SyncOutcome.Success
        }
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `the account screen shows syncing while a sync runs, then settles`() = runTest(dispatcher) {
        val controller = FakeSyncController()
        val vm = SyncViewModel(controller)

        vm.syncNow()
        runCurrent()
        // The user sees the sync in progress…
        assertEquals(SyncStatus.Syncing, vm.status.value)

        controller.gate.complete(Unit)
        advanceUntilIdle()
        // …and the screen settles back to idle when it finishes.
        assertEquals(SyncStatus.Idle, vm.status.value)
    }
}
