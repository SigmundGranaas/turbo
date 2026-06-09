package com.sigmundgranaas.turbo.expressive.domain

/** How the app picks light vs dark colours. */
enum class ThemeMode { System, Light, Dark }

/** Persisted user preferences (DataStore-backed). */
data class UserSettings(
    val compassOrientation: Boolean = true,
    val followLocation: Boolean = false,
    val metricUnits: Boolean = true,
    val themeMode: ThemeMode = ThemeMode.System,
    /** When off, the cloud sync engine is paused even while signed in. */
    val cloudSyncEnabled: Boolean = true,
    /** When on, offline map downloads only run on un-metered (Wi-Fi) networks. */
    val downloadOverWifiOnly: Boolean = false,
)
