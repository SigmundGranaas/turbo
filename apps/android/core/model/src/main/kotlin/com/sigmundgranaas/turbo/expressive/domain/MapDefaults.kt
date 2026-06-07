package com.sigmundgranaas.turbo.expressive.domain

/**
 * Map startup defaults. The app centres on the device's real GPS fix as soon as
 * one is available (see the home map's first-load location flow); these values are
 * only the placeholder shown while locating, or the resting view when location is
 * unavailable/denied — a neutral Norway-wide overview, not a fabricated place.
 */
object MapDefaults {
    /** Pre-GPS placeholder centre — a country-level view of Norway. */
    val fallbackCamera = LatLng(64.5, 13.5)
    const val fallbackZoom = 4.0
}
