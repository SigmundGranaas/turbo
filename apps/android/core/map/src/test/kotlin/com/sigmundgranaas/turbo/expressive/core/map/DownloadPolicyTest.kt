package com.sigmundgranaas.turbo.expressive.core.map

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DownloadPolicyTest {

    private val wifi = NetworkState(connected = true, unmetered = true)
    private val cellular = NetworkState(connected = true, unmetered = false)

    @Test
    fun `wifi-only off allows any connected network`() {
        assertTrue(DownloadPolicy.shouldDownload(wifiOnly = false, net = wifi))
        assertTrue(DownloadPolicy.shouldDownload(wifiOnly = false, net = cellular))
    }

    @Test
    fun `wifi-only on allows un-metered but blocks metered`() {
        assertTrue(DownloadPolicy.shouldDownload(wifiOnly = true, net = wifi))
        assertFalse(DownloadPolicy.shouldDownload(wifiOnly = true, net = cellular))
    }

    @Test
    fun `no connectivity always blocks`() {
        assertFalse(DownloadPolicy.shouldDownload(wifiOnly = false, net = NetworkState.Disconnected))
        assertFalse(DownloadPolicy.shouldDownload(wifiOnly = true, net = NetworkState.Disconnected))
    }
}
