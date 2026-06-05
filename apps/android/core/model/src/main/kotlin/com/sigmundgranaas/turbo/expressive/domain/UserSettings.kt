package com.sigmundgranaas.turbo.expressive.domain

/** How the app picks light vs dark colours. */
enum class ThemeMode { System, Light, Dark }

/** Persisted user preferences (DataStore-backed). */
data class UserSettings(
    val compassOrientation: Boolean = true,
    val followLocation: Boolean = false,
    val metricUnits: Boolean = true,
    val themeMode: ThemeMode = ThemeMode.System,
)
