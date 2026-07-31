package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.withTimeoutOrNull

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
) : RouteRepository {

    override fun planStream(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
        roundTrip: Boolean,
    ): Flow<RouteStreamEvent> = flow {
        val canFallBack = deviceCanAnswer(points)

        // Offline with a pack: skip the server entirely. Waiting out a
        // timeout we already know will expire is dead time in front of an
        // answer we could have given immediately.
        if (!isOnline() && canFallBack) {
            device.planStream(points, preset, profile, roundTrip).collect { emit(it) }
            return@flow
        }

        // No pack to fall back on: the server is the only answer there
        // is, so it gets no timeout. Cutting it short would turn a slow
        // route into no route, which is strictly worse.
        if (!canFallBack) {
            server.planStream(points, preset, profile, roundTrip).collect { emit(it) }
            return@flow
        }

        // Both available. Collect the server's events into a buffer under
        // a deadline, and only emit them once it has produced something
        // terminal — otherwise a half-streamed route would already be on
        // screen when the fallback takes over, and the user would watch a
        // line appear, vanish, and reappear differently.
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
            buffered.forEach { emit(it) }
        } else {
            device.planStream(points, preset, profile, roundTrip).collect { emit(it) }
        }
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
    }
}
