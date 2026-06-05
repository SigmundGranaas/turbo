package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.abs

/** "69.6412° N, 20.1003° E" — shared by the marker sheet and the detail host. */
fun formatCoords(p: LatLng): String {
    val ns = if (p.lat >= 0) "N" else "S"
    val ew = if (p.lng >= 0) "E" else "W"
    return "%.4f° %s, %.4f° %s".format(abs(p.lat), ns, abs(p.lng), ew)
}
