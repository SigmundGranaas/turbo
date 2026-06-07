package com.sigmundgranaas.turbo.expressive.core.geo

import kotlin.math.roundToInt

/**
 * Distance / elevation / pace formatting that honours the user's metric-vs-imperial
 * preference. Pure (no Android/Compose) so it's shared across every surface that
 * shows measurements — the UI reads the live preference from `LocalMetricUnits`.
 */
object Units {

    private const val METERS_PER_MILE = 1609.344
    private const val FEET_PER_METER = 3.280839895

    /** A distance for headline display: "1.4 km" / "0.9 mi" (or m/ft when short). */
    fun distance(meters: Double, metric: Boolean): String = if (metric) {
        if (meters >= 1000.0) "%.1f km".format(meters / 1000.0) else "${meters.roundToInt()} m"
    } else {
        val miles = meters / METERS_PER_MILE
        if (miles >= 0.1) "%.1f mi".format(miles) else "${(meters * FEET_PER_METER).roundToInt()} ft"
    }

    /** A vertical distance (ascent/descent/altitude): "742 m" / "2434 ft". */
    fun elevation(meters: Double, metric: Boolean): String =
        if (metric) "${meters.roundToInt()} m" else "${(meters * FEET_PER_METER).roundToInt()} ft"

    /** Ground speed from m/s: "9.1" km/h (metric) or mph (imperial). Unit-less number. */
    fun speedValue(metersPerSecond: Double, metric: Boolean): String {
        val v = if (metric) metersPerSecond * 3.6 else metersPerSecond * 3.6 / 1.609344
        return "%.1f".format(v)
    }

    /** The speed unit label that pairs with [speedValue]: "km/h" / "mph". */
    fun speedUnit(metric: Boolean): String = if (metric) "km/h" else "mph"

    /** Pace as time-per-unit: "5:30 /km" / "8:51 /mi". Returns "—" when undefined. */
    fun pace(distanceMeters: Double, seconds: Int, metric: Boolean): String {
        if (distanceMeters < 1.0 || seconds <= 0) return "—"
        val unitMeters = if (metric) 1000.0 else METERS_PER_MILE
        val secPerUnit = seconds / (distanceMeters / unitMeters)
        val m = (secPerUnit / 60).toInt()
        val s = (secPerUnit % 60).toInt()
        return "%d:%02d /%s".format(m, s, if (metric) "km" else "mi")
    }
}
