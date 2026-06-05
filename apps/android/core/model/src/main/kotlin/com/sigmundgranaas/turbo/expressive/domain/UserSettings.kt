package com.sigmundgranaas.turbo.expressive.domain

/** Persisted user preferences (DataStore-backed). */
data class UserSettings(
    val compassOrientation: Boolean = true,
    val followLocation: Boolean = false,
    val metricUnits: Boolean = true,
)
