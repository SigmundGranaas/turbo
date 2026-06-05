package com.sigmundgranaas.turbo.expressive.core.data

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Bundle
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import javax.inject.Inject

/**
 * Device location, sourced from the framework [LocationManager] (no Google Play
 * Services dependency). [locationUpdates] is a cold flow of fixes — it requires
 * the location permission to already be granted; otherwise it completes empty.
 */
interface LocationRepository {
    fun hasPermission(): Boolean
    fun locationUpdates(): Flow<LatLng>
}

class AndroidLocationRepository @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : LocationRepository {

    private val manager: LocationManager
        get() = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager

    override fun hasPermission(): Boolean =
        context.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            context.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED

    @SuppressLint("MissingPermission")
    override fun locationUpdates(): Flow<LatLng> = callbackFlow {
        if (!hasPermission()) {
            close()
            return@callbackFlow
        }
        val listener = object : LocationListener {
            override fun onLocationChanged(location: Location) {
                trySend(LatLng(location.latitude, location.longitude))
            }
            override fun onStatusChanged(provider: String?, status: Int, extras: Bundle?) = Unit
            override fun onProviderEnabled(provider: String) = Unit
            override fun onProviderDisabled(provider: String) = Unit
        }

        val providers = buildList {
            if (manager.isProviderEnabled(LocationManager.GPS_PROVIDER)) add(LocationManager.GPS_PROVIDER)
            if (manager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)) add(LocationManager.NETWORK_PROVIDER)
        }
        // Seed with the last known fix so the dot appears immediately.
        providers.firstNotNullOfOrNull { manager.getLastKnownLocation(it) }
            ?.let { trySend(LatLng(it.latitude, it.longitude)) }

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
