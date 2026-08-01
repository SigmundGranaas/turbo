package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.RouteDiagnostics
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.domain.RouteEngine
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveRecord
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import com.sigmundgranaas.turbo.expressive.domain.SolveLane
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Which router answers, and when.
 *
 * Every case here is a decision with a cost attached, so each is checked
 * by *who was asked* rather than by the route that came back — two
 * routers returning plausible routes look identical from the outside,
 * which is precisely why a wrong policy would ship unnoticed.
 */
class FallbackRouteRepositoryTest {

    private val points = listOf(LatLng(67.06, 15.04), LatLng(67.07, 15.05))

    private fun plan(distance: Double) = RoutePlan(
        distanceM = distance,
        durationS = 100.0,
        ascentM = 0.0,
        onTrailPct = 1.0,
        surfaces = emptyMap(),
        geometry = points,
    )

    /** A stand-in router that records being asked. */
    private class Fake(
        private val behaviour: suspend kotlinx.coroutines.flow.FlowCollector<RouteStreamEvent>.() -> Unit,
    ) : RouteRepository {
        var calls = 0
        override fun planStream(
            points: List<LatLng>,
            preset: RoutePreset,
            profile: String,
            roundTrip: Boolean,
        ): Flow<RouteStreamEvent> = flow {
            calls++
            behaviour()
        }
    }

    private fun answering(distance: Double, afterMs: Long = 0) = Fake {
        if (afterMs > 0) delay(afterMs)
        emit(RouteStreamEvent.Result(plan(distance)))
    }

    private fun hanging() = Fake { delay(Long.MAX_VALUE / 2) }

    private fun broken() = Fake { throw java.io.IOException("no route to host") }

    /**
     * The real class, with both routers faked at the `RouteRepository`
     * seam. What is under test is the POLICY, and it can be tested
     * exactly because the class asks for a predicate rather than for the
     * on-device repository — no native library, no pack on disk.
     */
    private fun repo(
        server: RouteRepository,
        device: RouteRepository,
        online: Boolean,
        hasPack: Boolean,
        timeoutMs: Long = 500,
        engine: RouteEngine = RouteEngine.Auto,
        diagnostics: RouteDiagnostics? = null,
        shadow: Boolean = false,
    ) = FallbackRouteRepository(
        server = server,
        device = device,
        isOnline = { online },
        deviceCanAnswer = { hasPack },
        serverTimeoutMs = timeoutMs,
        engineChoice = { engine },
        diagnostics = diagnostics,
        shadowCompare = { shadow },
    )

    private suspend fun collect(r: RouteRepository): List<RouteStreamEvent> =
        r.planStream(points, RoutePreset.Balanced, "foot", false).toList()

    @Test
    fun `online with a working server, the phone is never asked`() = runTest {
        val server = answering(1000.0)
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = true))

        assertEquals(1, server.calls)
        assertEquals("the device must not be woken when the server answered", 0, device.calls)
        assertEquals(1000.0, (events.single() as RouteStreamEvent.Result).plan.distanceM, 0.0)
    }

    @Test
    fun `offline with a pack, the server is never asked`() = runTest {
        // Not just correctness — waiting out a timeout we know will expire
        // is dead time in front of an answer already available.
        val server = hanging()
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = false, hasPack = true))

        assertEquals("no point asking a server we know is unreachable", 0, server.calls)
        assertEquals(1, device.calls)
        assertEquals(2000.0, (events.single() as RouteStreamEvent.Result).plan.distanceM, 0.0)
    }

    @Test
    fun `a hanging server is abandoned for the phone`() = runTest {
        // The case this class exists for: not "offline", but a validated
        // connection that accepts the handshake and then stops. The
        // request never fails, so nothing but a deadline saves the user.
        val server = hanging()
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = true))

        assertEquals(1, server.calls)
        assertEquals(1, device.calls)
        assertEquals(2000.0, (events.single() as RouteStreamEvent.Result).plan.distanceM, 0.0)
    }

    @Test
    fun `a network error falls through immediately`() = runTest {
        val server = broken()
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = true))
        assertEquals(1, device.calls)
        assertTrue(events.single() is RouteStreamEvent.Result)
    }

    @Test
    fun `a server that says there is no route is believed`() = runTest {
        // "No route through this terrain" is an answer about the WORLD,
        // not about the transport. Retrying on the phone would spend a
        // second to be told the same thing, and would make a real answer
        // look like a glitch worth retrying.
        val server = Fake { emit(RouteStreamEvent.Failure("No route through this terrain.")) }
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = true))

        assertEquals("a routing failure is not a transport failure", 0, device.calls)
        assertTrue(events.single() is RouteStreamEvent.Failure)
    }

    @Test
    fun `with no pack, a slow server is waited for rather than cut off`() = runTest {
        // There is nothing to fall back TO, so a deadline could only turn
        // a slow route into no route.
        val server = answering(1000.0, afterMs = 2_000)
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = false, timeoutMs = 100))

        assertEquals(0, device.calls)
        assertEquals(1000.0, (events.single() as RouteStreamEvent.Result).plan.distanceM, 0.0)
    }

    @Test
    fun `a partial server stream is not shown before the fallback takes over`() = runTest {
        // Without buffering, the user would watch a line appear, vanish,
        // and reappear differently — which reads as a bug even though
        // both routes are fine.
        val server = Fake {
            emit(RouteStreamEvent.Progress(points))
            delay(Long.MAX_VALUE / 2)
        }
        val device = answering(2000.0)
        val events = collect(repo(server, device, online = true, hasPack = true))

        assertTrue(
            "no half-finished server progress may reach the UI: $events",
            events.none { it is RouteStreamEvent.Progress },
        )
        assertEquals(2000.0, (events.single() as RouteStreamEvent.Result).plan.distanceM, 0.0)
    }

    // ── The overrides M1 is measured through ──────────────────────────

    @Test
    fun `forcing the phone asks only the phone, even with a good server`() = runTest {
        // The reason the override exists. Under Auto a healthy server
        // answers first every time, so a tester on a working connection
        // could run routes all day and never once exercise the engine
        // they are trying to measure.
        val server = answering(1000.0)
        val device = answering(2000.0)
        val events = collect(
            repo(server, device, online = true, hasPack = true, engine = RouteEngine.Device),
        )
        assertEquals("the server must not be consulted", 0, server.calls)
        assertEquals(1, device.calls)
        assertEquals(2000.0, (events.last() as RouteStreamEvent.Result).plan.distanceM, 0.001)
    }

    @Test
    fun `forcing the phone does NOT fall back when the phone fails`() = runTest {
        // The whole value of the override. A silent fallback would turn
        // "the device engine is broken in this APK" into "routing works",
        // which is the exact wrong answer to the only question being
        // asked.
        val server = answering(1000.0)
        val device = broken()
        val events = collect(
            repo(server, device, online = true, hasPack = true, engine = RouteEngine.Device),
        )
        assertEquals("a failure must stay visible", 0, server.calls)
        assertTrue("expected a Failure, got $events", events.last() is RouteStreamEvent.Failure)
    }

    @Test
    fun `forcing the server skips the phone even offline with a pack`() = runTest {
        val server = answering(1000.0)
        val device = answering(2000.0)
        collect(repo(server, device, online = false, hasPack = true, engine = RouteEngine.Server))
        assertEquals(1, server.calls)
        assertEquals("the control case must be a clean control", 0, device.calls)
    }

    // ── What gets recorded ────────────────────────────────────────────

    @Test
    fun `a solve is recorded against the engine that actually answered`() = runTest {
        // Not the engine that was asked first. Under Auto with a dead
        // server the phone answers, and charging that solve to the
        // server would corrupt the one number this exists to produce.
        val d = RouteDiagnostics()
        collect(
            repo(broken(), answering(2000.0), online = true, hasPack = true, diagnostics = d),
        )
        val r = d.records.value.single()
        assertEquals(RouteEngine.Device, r.engine)
        assertEquals(RouteSolveRecord.Outcome.Ok, r.outcome)
        assertEquals(2, r.waypoints)
        assertTrue("span should be a real distance, got ${r.spanKm}", r.spanKm > 0.0)
    }

    @Test
    fun `a native failure is recorded as Failed, with its message`() = runTest {
        // The release-APK case this readout exists for. A stripped .so
        // throws UnsatisfiedLinkError — an Error, not an Exception — so
        // a `catch (Exception)` would let it escape as a crash with no
        // attribution. It has to land here, legibly.
        val d = RouteDiagnostics()
        val device = Fake { throw UnsatisfiedLinkError("libturbo_route_ffi.so not found") }
        val events = collect(
            repo(answering(1.0), device, online = true, hasPack = true,
                engine = RouteEngine.Device, diagnostics = d),
        )
        val r = d.records.value.single()
        assertEquals(RouteSolveRecord.Outcome.Failed, r.outcome)
        assertTrue("the message must survive: ${r.detail}", r.detail!!.contains("libturbo_route_ffi"))
        assertTrue(events.last() is RouteStreamEvent.Failure)
    }

    @Test
    fun `a device solve is not charged for the server's timeout`() = runTest {
        // Under Auto the phone only runs after the server has burned the
        // whole window. Folding that into the device's duration would
        // make the phone look slower the worse the network is — exactly
        // backwards, and it would poison the p95 release 2 rests on.
        val d = RouteDiagnostics()
        collect(
            repo(hanging(), answering(2000.0), online = true, hasPack = true,
                timeoutMs = 300, diagnostics = d),
        )
        val r = d.records.value.single()
        assertEquals(RouteEngine.Device, r.engine)
        assertTrue(
            "device duration ${r.durationMs} ms must exclude the 300 ms server window",
            r.durationMs < 300,
        )
    }

    // ---- lanes -------------------------------------------------------
    //
    // The lane is WHY an engine ran, and every rate in RouteSolveStats is
    // derived from it. A wrong lane does not fail anything — it silently
    // moves a solve into or out of a denominator, so each branch of the
    // policy gets its own assertion here.

    @Test
    fun `a server answer inside the window is lane ServerAnswered`() = runTest {
        val d = RouteDiagnostics()
        collect(repo(answering(1000.0), answering(2.0), online = true, hasPack = true, diagnostics = d))
        assertEquals(SolveLane.ServerAnswered, d.records.value.single().lane)
    }

    @Test
    fun `a timeout that wakes the phone is lane ServerTimedOut`() = runTest {
        val d = RouteDiagnostics()
        collect(
            repo(hanging(), answering(2000.0), online = true, hasPack = true,
                timeoutMs = 200, diagnostics = d),
        )
        val r = d.records.value.single()
        assertEquals(RouteEngine.Device, r.engine)
        assertEquals(SolveLane.ServerTimedOut, r.lane)
        assertTrue(r.lane.isFallback)
    }

    /**
     * A transport error is a DIFFERENT lane from a timeout, even though
     * both end up on the phone. Tuning the timeout fixes one and not the
     * other, so a metric that merged them would point at the wrong knob.
     */
    @Test
    fun `a broken server is lane ServerErrored not ServerTimedOut`() = runTest {
        val d = RouteDiagnostics()
        collect(repo(broken(), answering(2000.0), online = true, hasPack = true, diagnostics = d))
        val r = d.records.value.single()
        assertEquals(RouteEngine.Device, r.engine)
        assertEquals(SolveLane.ServerErrored, r.lane)
        assertTrue(r.lane.isFallback)
    }

    @Test
    fun `offline with a pack is lane Offline and never a fallback`() = runTest {
        val d = RouteDiagnostics()
        collect(repo(answering(1.0), answering(2000.0), online = false, hasPack = true, diagnostics = d))
        val r = d.records.value.single()
        assertEquals(SolveLane.Offline, r.lane)
        assertTrue("no server was tried, so nothing was rescued", !r.lane.isFallback)
    }

    /** The coverage-miss signal: routed somewhere no pack covers. */
    @Test
    fun `no pack is lane NoPack`() = runTest {
        val d = RouteDiagnostics()
        collect(repo(answering(1000.0), answering(2.0), online = true, hasPack = false, diagnostics = d))
        val r = d.records.value.single()
        assertEquals(RouteEngine.Server, r.engine)
        assertEquals(SolveLane.NoPack, r.lane)
    }

    @Test
    fun `an override is lane Forced on either engine`() = runTest {
        val dev = RouteDiagnostics()
        collect(
            repo(answering(1.0), answering(2.0), online = true, hasPack = true,
                engine = RouteEngine.Device, diagnostics = dev),
        )
        assertEquals(SolveLane.Forced, dev.records.value.single().lane)

        val srv = RouteDiagnostics()
        collect(
            repo(answering(1.0), answering(2.0), online = true, hasPack = true,
                engine = RouteEngine.Server, diagnostics = srv),
        )
        assertEquals(SolveLane.Forced, srv.records.value.single().lane)
    }

    /**
     * A failed solve must still carry its lane.
     *
     * The failure path builds its own record, so it is the one place a
     * lane can be dropped without any other test noticing.
     */
    @Test
    fun `a failure keeps the lane it failed in`() = runTest {
        val d = RouteDiagnostics()
        val device = Fake { throw UnsatisfiedLinkError("stripped") }
        collect(
            repo(hanging(), device, online = true, hasPack = true,
                timeoutMs = 100, diagnostics = d),
        )
        val r = d.records.value.single()
        assertEquals(RouteSolveRecord.Outcome.Failed, r.outcome)
        assertEquals(SolveLane.ServerTimedOut, r.lane)
    }

    // ---- shadow comparison -------------------------------------------

    /** Off by default: the second engine must not be woken. */
    @Test
    fun `shadow comparison is off unless asked for`() = runTest {
        val d = RouteDiagnostics()
        val device = answering(2000.0)
        collect(repo(answering(1000.0), device, online = true, hasPack = true, diagnostics = d))
        assertEquals("the phone must not run for a metric nobody asked for", 0, device.calls)
        assertEquals(null, d.records.value.single().divergence)
    }

    /** With it on, both run and the gap is recorded. */
    @Test
    fun `shadow comparison records how far apart the engines were`() = runTest {
        val d = RouteDiagnostics()
        val device = answering(2000.0)
        collect(
            repo(answering(1000.0), device, online = true, hasPack = true,
                diagnostics = d, shadow = true),
        )
        assertEquals(1, device.calls)
        val div = d.records.value.single().divergence
        assertTrue("a divergence should have been recorded", div != null)
        // Both fakes return the same geometry, so the lines agree even
        // though the reported distances differ — which is the point of
        // comparing geometry rather than trusting the plan's number.
        assertEquals(0.0, div!!.frechetM, 1e-6)
    }

    /**
     * The shadow solve must not delay the route.
     *
     * If the comparison ran before the answer was emitted, a slow second
     * engine would hold up a route the user could already have had —
     * turning a diagnostic into a regression.
     */
    @Test
    fun `the real answer is emitted before the shadow engine runs`() = runTest {
        val order = mutableListOf<String>()
        val device = Fake {
            order += "shadow"
            emit(RouteStreamEvent.Result(plan(2000.0)))
        }
        val r = repo(answering(1000.0), device, online = true, hasPack = true, shadow = true)
        r.planStream(points, RoutePreset.Balanced, "foot", false).collect {
            if (it is RouteStreamEvent.Result) order += "answer"
        }
        assertEquals(listOf("answer", "shadow"), order)
    }

    /**
     * A broken shadow engine must not break the route.
     *
     * A diagnostic that can fail a solve is worse than no diagnostic.
     */
    @Test
    fun `a failing shadow engine leaves the route intact`() = runTest {
        val d = RouteDiagnostics()
        val device = Fake { throw java.io.IOException("shadow exploded") }
        val events = collect(
            repo(answering(1000.0), device, online = true, hasPack = true,
                diagnostics = d, shadow = true),
        )
        assertTrue("the route must survive", events.any { it is RouteStreamEvent.Result })
        val r = d.records.value.single()
        assertEquals(RouteSolveRecord.Outcome.Ok, r.outcome)
        assertEquals("no comparison rather than a fake one", null, r.divergence)
    }

    /** No pack means the device could not have answered — nothing to compare. */
    @Test
    fun `shadow comparison is skipped when the device has no pack`() = runTest {
        val d = RouteDiagnostics()
        val device = answering(2000.0)
        collect(
            repo(answering(1000.0), device, online = true, hasPack = false,
                diagnostics = d, shadow = true),
        )
        assertEquals(0, device.calls)
        assertEquals(null, d.records.value.single().divergence)
    }
}
