package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * The current metric-vs-imperial preference, provided once at the app root from
 * user settings. UI reads it (`LocalMetricUnits.current`) and formats measurements
 * via `com.sigmundgranaas.turbo.expressive.core.geo.Units`. Defaults to metric.
 */
val LocalMetricUnits = staticCompositionLocalOf { true }
