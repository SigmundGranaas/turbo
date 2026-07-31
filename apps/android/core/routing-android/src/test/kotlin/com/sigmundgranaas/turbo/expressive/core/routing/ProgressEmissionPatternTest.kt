package com.sigmundgranaas.turbo.expressive.core.routing

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.atomic.AtomicReference

/**
 * The shape [OnDeviceRouteRepository] streams progress with, on its own.
 *
 * A blocking solve runs in `async` while the flow's own coroutine polls
 * the newest snapshot and emits it. Kotlin enforces that a flow emits
 * only from the collector's context — "Flow invariant is violated" — and
 * whether `coroutineScope` counts as the same context is not something
 * to settle by reading the docs when the failure mode is a crash on a
 * device, mid-hike, the first time a route is planned offline.
 */
class ProgressEmissionPatternTest {

    @Test
    fun `emitting from coroutineScope while a child works does not violate the flow contract`() =
        runTest {
            val latest = AtomicReference<Int?>(null)

            val events = flow {
                coroutineScope {
                    val work = async(Dispatchers.Default) {
                        repeat(5) {
                            latest.set(it)
                            Thread.sleep(20)
                        }
                        "done"
                    }
                    while (work.isActive) {
                        latest.getAndSet(null)?.let { emit(it) }
                        delay(5)
                    }
                    // Anything the poll loop missed between its last read
                    // and the solve finishing.
                    latest.getAndSet(null)?.let { emit(it) }
                    emit(-1) // stands in for the terminal Result
                    work.await()
                }
            }.flowOn(Dispatchers.Default).toList()

            assertTrue("progress must reach the collector: $events", events.size > 1)
            assertEquals("the terminal event must arrive last", -1, events.last())
        }
}
