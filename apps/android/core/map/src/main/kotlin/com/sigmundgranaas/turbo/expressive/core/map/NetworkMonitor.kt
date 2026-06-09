package com.sigmundgranaas.turbo.expressive.core.map

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

/** Observes the device's connectivity as a [NetworkState] stream. */
interface NetworkMonitor {
    val state: StateFlow<NetworkState>
}

/**
 * [NetworkMonitor] backed by [ConnectivityManager]'s default-network callback.
 * Singleton: it registers once and keeps a hot [StateFlow] the download service
 * reads to gate (and resume) downloads as Wi-Fi/cellular comes and goes.
 */
@Singleton
class AndroidNetworkMonitor @Inject constructor(
    @param:ApplicationContext context: Context,
) : NetworkMonitor {

    private val cm = context.getSystemService(ConnectivityManager::class.java)
    private val _state = MutableStateFlow(snapshot())
    override val state: StateFlow<NetworkState> = _state.asStateFlow()

    init {
        cm?.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) { _state.value = snapshot() }
            override fun onLost(network: Network) { _state.value = NetworkState.Disconnected }
            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                _state.value = caps.toState()
            }
        })
    }

    private fun snapshot(): NetworkState {
        val caps = cm?.getNetworkCapabilities(cm.activeNetwork) ?: return NetworkState.Disconnected
        return caps.toState()
    }

    private fun NetworkCapabilities.toState(): NetworkState {
        val online = hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
            hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
        val unmetered = hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
        return NetworkState(connected = online, unmetered = unmetered)
    }
}
