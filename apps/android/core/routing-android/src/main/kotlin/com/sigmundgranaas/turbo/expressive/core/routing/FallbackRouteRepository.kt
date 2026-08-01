package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.RouteDiagnostics
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RouteEngine
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteSolveRecord
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.abs
import kotlin.math.cos

/**
 * Server first; the phone when the server cannot answer.
 *
 * # Why this way round, for now
 *
 * On-device routing is the better end state — predictable, no radio, no
 * data, and it works when connectivity *lies*, which in a valley it
 * routinely does.
 *
 * It used to be visibly worse in one way: the façade's `plan` was a
 * single blocking call, so a route appeared rather than drew. That gap
 * is closed — `planWithProgress` streams the same best-path snapshots
 * the server's SSE endpoint reads off the same solver hook — so the
 * argument for leading with the server is now down to ONE thing, and it
 * is an absence of evidence rather than a known deficit: **nobody has
 * measured a solve on real hardware.** Every latency figure quoted for
 * the device path is extrapolated from a desktop.
 *
 * Until someone runs a route on a phone, leading with the server keeps
 * this release **strictly additive**: nobody who is happy today gets a
 * worse experience, and the device path earns its trust on requests that
 * currently fail outright. Flipping the order is a one-line change here
 * and [the plan][1] — but it should follow a measurement, not a hunch.
 *
 * [1]: apps/android/docs/on-device-routing-ux-plan.md
 *
 * # The timeout is the point
 *
 * Not the offline case — that one is easy, and `NetworkMonitor` already
 * reports it. The case this exists for is a *validated* connection that
 * does not work: one bar, a captive portal, a tower that accepts the TCP
 * handshake and then stops. The request does not fail, it hangs, and a
 * router that waits for it is worse than one that never tried. So the
 * server gets a bounded window and the phone answers after it.
 */
class FallbackRouteRepository(
    private val server: RouteRepository,
    private val device: RouteRepository,
    private val isOnline: () -> Boolean,
    /**
     * Can the device answer for these points — i.e. does a downloaded
     * pack contain all of them?
     *
     * A predicate rather than a call on [OnDeviceRouteRepository],
     * because this class decides *which* router runs and has no business
     * knowing what either one is. It also means the policy can be tested
     * without a native library on the other end, which is the difference
     * between these rules being checked and being asserted in a comment.
     */
    private val deviceCanAnswer: (List<LatLng>) -> Boolean,
    private val serverTimeoutMs: Long = DEFAULT_SERVER_TIMEOUT_MS,
    /**
     * Which engine to use, read per request.
     *
     * A function rather than a value because the setting can change
     * between one route and the next, and a tester toggling it wants the
     * next route to obey — not the next app launch.
     */
    private val engineChoice: () -> RouteEngine = { RouteEngine.Auto },
    /**
     * Where each solve's cost is reported. Defaults to dropping it: the
     * policy this class implements does not depend on anyone listening.
     */
    private val diagnostics: RouteDiagnostics? = null,
    private val nowMs: () -> Long = System::currentTimeMillis,
) : RouteRepository {

    override fun planStream(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
        roundTrip: Boolean,
    ): Flow<RouteStreamEvent> = flow {
        val canFallBack = deviceCanAnswer(points)

        // An explicit override runs exactly one engine and reports what
        // it did. No fallback: the whole point of forcing the device is
        // to find out whether the device works, and an answer quietly
        // supplied by the server would say the opposite of the truth.
        when (engineChoice()) {
            RouteEngine.Device -> {
                emitRecorded(RouteEngine.Device, points) {
                    device.planStream(points, preset, profile, roundTrip)
                }
                return@flow
            }
            RouteEngine.Server -> {
                emitRecorded(RouteEngine.Server, points) {
                    server.planStream(points, preset, profile, roundTrip)
                }
                return@flow
            }
            RouteEngine.Auto -> Unit
        }

        // Offline with a pack: skip the server entirely. Waiting out a
        // timeout we already know will expire is dead time in front of an
        // answer we could have given immediately.
        if (!isOnline() && canFallBack) {
            emitRecorded(RouteEngine.Device, points) {
                device.planStream(points, preset, profile, roundTrip)
            }
            return@flow
        }

        // No pack to fall back on: the server is the only answer there
        // is, so it gets no timeout. Cutting it short would turn a slow
        // route into no route, which is strictly worse.
        if (!canFallBack) {
            emitRecorded(RouteEngine.Server, points) {
                server.planStream(points, preset, profile, roundTrip)
            }
            return@flow
        }

        // Both available. Collect the server's events into a buffer under
        // a deadline, and only emit them once it has produced something
        // terminal — otherwise a half-streamed route would already be on
        // screen when the fallback takes over, and the user would watch a
        // line appear, vanish, and reappear differently.
        val startedAt = nowMs()
        val buffered = withTimeoutOrNull(serverTimeoutMs) {
            val events = mutableListOf<RouteStreamEvent>()
            var ok = false
            try {
                server.planStream(points, preset, profile, roundTrip).collect { e ->
                    events += e
                    if (e is RouteStreamEvent.Result) ok = true
                }
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (_: Exception) {
                // Network error: fall through to the device, which is the
                // whole reason this class exists.
                return@withTimeoutOrNull null
            }
            // A Failure from the server is a real answer about the ROUTE
            // ("no route through this terrain"), not a transport problem,
            // and the device would only say the same thing more slowly.
            if (ok || events.any { it is RouteStreamEvent.Failure }) events else null
        }

        if (buffered != null) {
            record(RouteEngine.Server, points, nowMs() - startedAt, buffered)
            buffered.forEach { emit(it) }
        } else {
            // The server's spent time is NOT charged to the device here.
            // The number this exists to produce is "how long does the
            // phone take", and folding a 4 s timeout into it would make
            // the device path look worse the slower the network is —
            // exactly backwards.
            emitRecorded(RouteEngine.Device, points) {
                device.planStream(points, preset, profile, roundTrip)
            }
        }
    }

    /**
     * Run one engine, forward its events, and report what it cost.
     *
     * Catches [Throwable], not [Exception], and that is the important
     * part. A release APK whose native library was stripped by R8, or
     * whose `.so` is missing for this ABI, throws
     * `UnsatisfiedLinkError` — an `Error`. Letting it propagate would
     * surface as a crash with no attribution; swallowing it would be
     * worse. Recording it as [RouteSolveRecord.Outcome.Failed] with the
     * message is what makes a packaging defect legible on the phone
     * rather than in a stack trace nobody sees.
     */
    private suspend fun kotlinx.coroutines.flow.FlowCollector<RouteStreamEvent>.emitRecorded(
        engine: RouteEngine,
        points: List<LatLng>,
        source: () -> Flow<RouteStreamEvent>,
    ) {
        val startedAt = nowMs()
        val seen = mutableListOf<RouteStreamEvent>()
        try {
            source().collect { e ->
                seen += e
                emit(e)
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (t: Throwable) {
            diagnostics?.record(
                RouteSolveRecord(
                    engine = engine,
                    durationMs = nowMs() - startedAt,
                    waypoints = points.size,
                    spanKm = spanKm(points),
                    outcome = RouteSolveRecord.Outcome.Failed,
                    detail = t.message ?: t::class.java.simpleName,
                ),
            )
            emit(RouteStreamEvent.Failure(t.message ?: "Routing failed on this device."))
            return
        }
        record(engine, points, nowMs() - startedAt, seen)
    }

    private fun record(
        engine: RouteEngine,
        points: List<LatLng>,
        durationMs: Long,
        events: List<RouteStreamEvent>,
    ) {
        val d = diagnostics ?: return
        val failure = events.filterIsInstance<RouteStreamEvent.Failure>().lastOrNull()
        d.record(
            RouteSolveRecord(
                engine = engine,
                durationMs = durationMs,
                waypoints = points.size,
                spanKm = spanKm(points),
                outcome = when {
                    events.any { it is RouteStreamEvent.Result } -> RouteSolveRecord.Outcome.Ok
                    failure != null -> RouteSolveRecord.Outcome.NoRoute
                    else -> RouteSolveRecord.Outcome.Failed
                },
                detail = failure?.message,
            ),
        )
    }

    /**
     * Straight-line span across the request, for bucketing.
     *
     * The bounding box's diagonal rather than the sum of legs: it is
     * defined for a failed solve, where there are no legs yet, and it is
     * what actually predicts the search area the solver has to cover.
     */
    private fun spanKm(points: List<LatLng>): Double {
        if (points.size < 2) return 0.0
        val lats = points.map { it.lat }
        val lngs = points.map { it.lng }
        val midLat = (lats.min() + lats.max()) / 2.0
        val dLat = abs(lats.max() - lats.min()) * KM_PER_DEG
        val dLng = abs(lngs.max() - lngs.min()) * KM_PER_DEG * cos(Math.toRadians(midLat))
        return kotlin.math.hypot(dLat, dLng)
    }



    companion object {
        /**
         * How long the server gets before the phone answers instead.
         *
         * A guess, and labelled as one: it should be set from measured
         * server latency once there is any. Four seconds is long enough
         * that an ordinary solve lands inside it and short enough that a
         * dead connection does not hold a walker still.
         */
        const val DEFAULT_SERVER_TIMEOUT_MS: Long = 4_000L

        private const val KM_PER_DEG = 111.320
    }
}
