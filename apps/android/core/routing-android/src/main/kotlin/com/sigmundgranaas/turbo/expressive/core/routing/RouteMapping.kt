package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset

/**
 * Translation between the app's vocabulary and the engine's, as pure functions.
 *
 * Deliberately free of anything native. Everything interesting about the
 * on-device path that can be wrong in an ordinary way — a swapped lat/lng, a
 * preset key that does not exist, an on-trail percentage computed off the wrong
 * denominator — lives here and is unit-testable on the JVM without an NDK, a
 * device, or a pack. What is left in [OnDeviceRouteRepository] is the part that
 * genuinely needs hardware.
 *
 * The swap risk is not hypothetical: the app's [LatLng] is `(lat, lng)` and the
 * engine's `GeoPoint` is `(lon, lat)`. They are both pairs of doubles in the
 * same numeric range over Norway, so a transposition compiles, runs, and puts
 * the route in the Barents Sea.
 */
internal object RouteMapping {

    /**
     * Which preset key the engine is asked for.
     *
     * [RoutePreset.key] is already the engine's name — the enum was written
     * against the same `route-presets.toml` — so this is a read, not a
     * translation table. Keeping it as a function anyway is what makes
     * [presetKeysAreEngineNames] able to fail if that ever stops being true.
     */
    fun presetKey(preset: RoutePreset): String = preset.key

    /**
     * Share of the route on a mapped trail, 0..1.
     *
     * "On trail" is every surface the engine names except `off_trail`, rather
     * than a list of the ones we know about: a new surface class in the graph
     * should count as trail by default, because it came from the trail network.
     * Listing the good ones would silently reclassify it as wilderness.
     */
    fun onTrailFraction(surfaces: Map<String, Double>): Double {
        val total = surfaces.values.sum()
        if (total <= 0.0) return 0.0
        val off = surfaces[OFF_TRAIL] ?: 0.0
        return ((total - off) / total).coerceIn(0.0, 1.0)
    }

    const val OFF_TRAIL: String = "off_trail"

    /**
     * Build the app's [RoutePlan] from the engine's numbers.
     *
     * `geometry` is passed already converted so this function stays free of the
     * generated types — see the note on this object.
     */
    fun plan(
        geometry: List<LatLng>,
        lengthM: Double,
        durationS: Double,
        ascentM: Double,
        surfaces: Map<String, Double>,
    ): RoutePlan = RoutePlan(
        distanceM = lengthM,
        durationS = durationS,
        ascentM = ascentM,
        onTrailPct = onTrailFraction(surfaces),
        surfaces = surfaces,
        geometry = geometry,
    )
}
