package com.sigmundgranaas.turbo.expressive.ui.map

import kotlin.math.cos
import kotlin.math.pow

/**
 * Pure scale-bar geometry. Picks a round real-world distance that fits within a
 * pixel budget at the given Web-Mercator zoom + latitude, and reports how wide
 * (px) to draw it. Isolated from MapLibre so it can be unit-tested.
 */
internal object ScaleBar {
    data class Spec(val meters: Double, val widthPx: Float, val label: String)

    /** Metres covered by one screen pixel at [latitude] and [zoom] (256-px tiles). */
    fun metersPerPixel(latitude: Double, zoom: Double): Double =
        EARTH_CIRCUMFERENCE_M * cos(Math.toRadians(latitude)) / (256.0 * 2.0.pow(zoom))

    /** A nice-round bar (1/2/5 × 10ⁿ) whose pixel width is ≤ [maxWidthPx]. */
    fun compute(latitude: Double, zoom: Double, maxWidthPx: Float): Spec {
        val mpp = metersPerPixel(latitude, zoom).coerceAtLeast(1e-6)
        val maxMeters = mpp * maxWidthPx
        val meters = niceDistance(maxMeters)
        return Spec(meters, (meters / mpp).toFloat(), label(meters))
    }

    private fun niceDistance(maxMeters: Double): Double {
        if (maxMeters <= 1.0) return 1.0
        val pow = 10.0.pow(kotlin.math.floor(kotlin.math.log10(maxMeters)))
        return when {
            5 * pow <= maxMeters -> 5 * pow
            2 * pow <= maxMeters -> 2 * pow
            else -> pow
        }
    }

    private fun label(meters: Double): String =
        if (meters >= 1000) "${(meters / 1000).toInt()} km" else "${meters.toInt()} m"

    private const val EARTH_CIRCUMFERENCE_M = 40_075_016.686
}
