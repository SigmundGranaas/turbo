package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import uniffi.turbo_route_ffi.GeoPoint
import uniffi.turbo_route_ffi.RouteEngine
import uniffi.turbo_route_ffi.RouteException
import uniffi.turbo_route_ffi.RouteOptions
import uniffi.turbo_route_ffi.TravelMode

/**
 * [RouteRepository] backed by the on-device engine instead of the HTTP API.
 *
 * The same interface `HttpRouteRepository` implements, so nothing above it —
 * `RouteViewModel`, the map feature, the route card — changes. Which one is
 * bound is a composition decision (see the Hilt module), and the honest
 * criterion is coverage: on-device when a downloaded pack contains every
 * waypoint, server otherwise. Not "when offline", because a user standing in a
 * covered region on a bad 2G connection is exactly who this is for.
 *
 * ## What this cannot do yet, and says so
 *
 * **No progress.** [RouteRepository.planStream] streams the solver's best-path
 * snapshots and the UI animates them. The façade's `plan` is one blocking call,
 * so this emits a single terminal event. The engine already produces the
 * snapshots — the server's SSE endpoint reads them off `solver_trace` — so
 * exposing them across FFI is a callback interface, not new solver work. Until
 * then a route appears rather than draws, which is a downgrade worth naming
 * rather than papering over with a fake animation.
 *
 * **No cancellation.** `plan` runs to completion; cancelling the coroutine
 * abandons the result but not the work. `RouteViewModel` already defers
 * re-solves while a waypoint is being dragged, which covers the worst case.
 *
 * ## Threading
 *
 * `plan` blocks for roughly a quarter of a second to several seconds depending
 * on lane and distance, so it runs on [dispatcher] and never on the main
 * thread. [RouteEngine] is `Sync` on the Rust side and uniffi hands out an
 * `Arc`, so one instance serves concurrent callers.
 */
class OnDeviceRouteRepository(
    private val packs: PackStore,
    private val dispatcher: CoroutineDispatcher = Dispatchers.Default,
) : RouteRepository {

    /**
     * One open engine per pack, kept for the process lifetime.
     *
     * Opening costs ~320 ms on a desktop, nearly all of it building the trail
     * R-trees; E4 measured 555 ms on a national graph. Re-opening per request is
     * the exact mistake the engine's architecture was reorganised to make
     * impossible, and it would be an odd place to reintroduce it.
     */
    private val engines = HashMap<String, RouteEngine>()

    @Synchronized
    private fun engineFor(pack: PackStore.Pack): RouteEngine =
        // `RouteEngine.open(...)`, not a constructor: uniffi maps a named Rust
        // constructor to a companion function, and `RouteEngine(x)` resolves to
        // the internal pointer constructor instead. Checked against the
        // generated bindings rather than assumed — the README had it wrong.
        engines.getOrPut(pack.id) { RouteEngine.open(pack.dir.absolutePath) }

    /** Is there a downloaded pack covering all of [points]? */
    fun canPlanOffline(points: List<LatLng>): Boolean =
        packs.covering(points.map { it.lng to it.lat }) != null

    override fun planStream(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
        roundTrip: Boolean,
    ): Flow<RouteStreamEvent> = flow {
        val pack = packs.covering(points.map { it.lng to it.lat })
            ?: run {
                emit(RouteStreamEvent.Failure("No downloaded map covers this route."))
                return@flow
            }

        val options = RouteOptions(
            mode = travelMode(profile),
            preset = RouteMapping.presetKey(preset),
            roundTrip = roundTrip,
        )
        // Everything else on RouteOptions defaults from Rust — including both
        // span budgets. Restating them here would freeze the app's copy at
        // whatever they were when this line was written, and a retune would
        // stop reaching the phone.

        val event = try {
            val route = engineFor(pack).plan(points.map { GeoPoint(it.lng, it.lat) }, options)
            RouteStreamEvent.Result(
                RouteMapping.plan(
                    geometry = route.geometry.map { LatLng(it.lat, it.lon) },
                    lengthM = route.lengthM,
                    durationS = route.durationS,
                    ascentM = route.ascentM,
                    surfaces = route.surfaceBreakdown.associate { it.surface to it.lengthM },
                ),
            )
        } catch (e: RouteException) {
            RouteStreamEvent.Failure(message(e))
        }
        emit(event)
    }.flowOn(dispatcher)

    private fun travelMode(profile: String): TravelMode = when (profile) {
        "bicycle" -> TravelMode.BICYCLE
        "ski" -> TravelMode.SKI
        else -> TravelMode.FOOT
    }

    /**
     * A message for a person, not the engine's.
     *
     * The variants are separate precisely because the user's next action
     * differs — download more map, versus move the pin off the lake, versus
     * shorten the leg — and collapsing them into "couldn't find a route" is what
     * makes an app feel broken. The engine's own text names files and
     * kilometres; useful in a log, not on a route card.
     */
    private fun message(e: RouteException): String = when (e) {
        is RouteException.OutsideCoverage -> "That's outside the map you've downloaded."
        is RouteException.EndpointBlocked -> "One of your points is somewhere you can't walk — water, maybe. Try moving it."
        is RouteException.TooLong -> "That's too far for one route. Try a shorter leg."
        is RouteException.NoRoute -> "No route through this terrain."
        is RouteException.Pack -> "The downloaded map couldn't be opened. Try downloading it again."
        is RouteException.InvalidRequest -> "That route request doesn't make sense."
        else -> "Something went wrong planning that route."
    }
}
