package com.sigmundgranaas.turbo.expressive.core.data

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Build
import android.os.Bundle
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

/**
 * A single GPS fix with optional altitude (metres above the WGS84 ellipsoid),
 * horizontal [accuracyM] (68 % radius in metres; null when unknown), and the
 * instantaneous ground [speedMps] reported by the provider (metres/second; null
 * when the fix carries no speed — e.g. a coarse network fix).
 */
data class LocationSample(
    val position: LatLng,
    val altitude: Double?,
    val accuracyM: Double? = null,
    val speedMps: Double? = null,
)

/**
 * Device location, sourced from the framework [LocationManager] (no Google Play
 * Services dependency). The flows are cold and require the location permission to
 * already be granted; otherwise they complete empty. [samples] carries altitude
 * (used to build recording elevation profiles); [locationUpdates] is the 2D
 * position only, derived from it.
 */
interface LocationRepository {
    fun hasPermission(): Boolean

    /**
     * Whether device location services (GPS / network providers) are switched on.
     * Distinct from [hasPermission]: the app may hold the permission while the user
     * has location turned off system-wide, in which case the flows never emit.
     */
    fun isLocationEnabled(): Boolean = true

    fun samples(): Flow<LocationSample>
    fun locationUpdates(): Flow<LatLng> = samples().map { it.position }
}

class AndroidLocationRepository @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : LocationRepository {

    private val manager: LocationManager
        get() = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager

    override fun hasPermission(): Boolean =
        context.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            context.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED

    override fun isLocationEnabled(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            manager.isLocationEnabled
        } else {
            manager.isProviderEnabled(LocationManager.GPS_PROVIDER) ||
                manager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)
        }

    @SuppressLint("MissingPermission")
    override fun samples(): Flow<LocationSample> = callbackFlow {
        if (!hasPermission()) {
            close()
            return@callbackFlow
        }
        fun Location.toSample() = LocationSample(
            LatLng(latitude, longitude),
            if (hasAltitude()) altitude else null,
            if (hasAccuracy()) accuracy.toDouble() else null,
            if (hasSpeed()) speed.toDouble() else null,
        )
        // Drop inaccurate / stale / teleporting fixes before anyone sees them — in
        // particular the (often stale) last-known seed below, the resume-teleport source.
        val filter = LocationFilter()
        var lastFixTime = 0L
        fun emitIfAccepted(location: Location) {
            val accuracyM = if (location.hasAccuracy()) location.accuracy.toDouble() else Double.MAX_VALUE
            val ageMs = (System.currentTimeMillis() - location.time).toDouble().coerceAtLeast(0.0)
            val intervalMs = if (lastFixTime > 0L) (location.time - lastFixTime).toDouble().coerceAtLeast(1.0) else 1000.0
            lastFixTime = location.time
            if (filter.accept(LatLng(location.latitude, location.longitude), accuracyM, ageMs, intervalMs)) {
                trySend(location.toSample())
            }
        }
        val listener = object : LocationListener {
            override fun onLocationChanged(location: Location) = emitIfAccepted(location)
            override fun onStatusChanged(provider: String?, status: Int, extras: Bundle?) = Unit
            override fun onProviderEnabled(provider: String) = Unit
            override fun onProviderDisabled(provider: String) = Unit
        }

        val providers = buildList {
            if (manager.isProviderEnabled(LocationManager.GPS_PROVIDER)) add(LocationManager.GPS_PROVIDER)
            if (manager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)) add(LocationManager.NETWORK_PROVIDER)
        }
        // Seed with the last known fix so the dot appears immediately — but only
        // if it's fresh/accurate enough (the filter drops a stale seed → no teleport).
        providers.firstNotNullOfOrNull { manager.getLastKnownLocation(it) }
            ?.let { emitIfAccepted(it) }

        providers.forEach { provider ->
            manager.requestLocationUpdates(provider, MIN_INTERVAL_MS, MIN_DISTANCE_M, listener, context.mainLooper)
        }
        awaitClose { manager.removeUpdates(listener) }
    }

    private companion object {
        const val MIN_INTERVAL_MS = 1_000L
        const val MIN_DISTANCE_M = 2f
    }
}
