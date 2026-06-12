package com.sigmundgranaas.turbo.expressive.core.map

/** A snapshot of the device's connectivity, as the download policy needs it. */
data class NetworkState(
    val connected: Boolean,
    /** True on un-metered networks (Wi-Fi / Ethernet); false on metered cellular. */
    val unmetered: Boolean,
) {
    companion object {
        val Disconnected = NetworkState(connected = false, unmetered = false)
    }
}

/**
 * Pure decision for whether offline downloads may run right now, given the user's
 * "Wi-Fi only" preference and the current [NetworkState]. Isolated so it is trivially
 * unit-testable; the connectivity plumbing lives in [NetworkMonitor].
 */
object DownloadPolicy {
    fun shouldDownload(wifiOnly: Boolean, net: NetworkState): Boolean =
        net.connected && (!wifiOnly || net.unmetered)
}
