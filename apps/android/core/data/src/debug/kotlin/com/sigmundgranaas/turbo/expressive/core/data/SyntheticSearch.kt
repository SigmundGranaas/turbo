package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import javax.inject.Inject
import kotlin.math.abs

/**
 * Offline stand-ins for the search family — Kartverket place search, Geonorge
 * trail search, and Kartverket/Høydedata reverse-geocode are all HTTP and Norway-
 * only, so on the emulator search returns nothing and tapping the map can't name
 * a place. These fabricate plausible, deterministic results from the query /
 * coordinate so the search screen, result selection, recents, and the long-press
 * "what's here" naming can be driven anywhere. Selected in DEBUG via NetworkModule.
 */
class SyntheticSearchRepository @Inject constructor() : SearchRepository {
    override suspend fun search(query: String): Outcome<List<SearchHit>> {
        val q = query.trim()
        if (q.length < 2) return Outcome.Success(emptyList())
        val cap = q.replaceFirstChar { it.uppercase() }
        val kinds = listOf(
            "fjellet" to "Mountain", "vatnet" to "Lake", " dalen" to "Valley",
            "hytta" to "Cabin", "toppen" to "Summit",
        )
        val hits = kinds.mapIndexed { i, (suffix, kind) ->
            SearchHit(
                name = "$cap$suffix",
                description = "$kind · Simulated kommune",
                position = LatLng(69.65 + i * 0.01, 18.95 + i * 0.013),
            )
        }
        return Outcome.Success(hits)
    }
}

class SyntheticTrailSearchRepository @Inject constructor() : TrailSearchRepository {
    override suspend fun search(query: String): Outcome<List<SearchHit>> {
        val q = query.trim()
        if (q.length < 2) return Outcome.Success(emptyList())
        val cap = q.replaceFirstChar { it.uppercase() }
        val routes = listOf("$cap-stien", "$cap rundt", "$cap til toppen")
        val hits = routes.mapIndexed { i, name ->
            SearchHit(
                name = name,
                description = "Marked trail · ${6 + i * 2}.4 km",
                position = LatLng(69.66 + i * 0.012, 18.92 + i * 0.01),
            )
        }
        return Outcome.Success(hits)
    }
}

class SyntheticReverseGeocodeRepository @Inject constructor() : ReverseGeocodeRepository {
    override suspend fun describe(point: LatLng): Outcome<LocationDescription> {
        // Deterministic pick from the coordinate so different taps read differently.
        val seed = ((abs(point.lat * 1000).toInt()) + (abs(point.lng * 1000).toInt()))
        val onAFeature = seed % 2 == 0
        val peaks = listOf("Storfjellet", "Blåtind", "Tromsdalstinden", "Rundfjellet")
        val places = listOf("Tromsø", "Lom", "Lyngen", "Kåfjord")
        val elevation = 200.0 + (abs(point.lat - 60.0) * 60.0) + (seed % 400)
        return Outcome.Success(
            if (onAFeature) {
                LocationDescription(
                    title = peaks[seed % peaks.size],
                    qualifier = PlaceQualifier.On,
                    secondary = "Mountain · ${places[seed % places.size]}",
                    elevationM = elevation,
                )
            } else {
                LocationDescription(
                    title = places[seed % places.size],
                    qualifier = PlaceQualifier.In,
                    secondary = "Kommune",
                    elevationM = elevation,
                )
            },
        )
    }
}
