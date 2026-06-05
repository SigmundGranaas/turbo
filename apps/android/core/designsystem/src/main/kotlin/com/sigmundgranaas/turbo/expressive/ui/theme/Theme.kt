package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.runtime.Composable

/**
 * Turbo's Material 3 Expressive theme: the warm-rust [TurboLightColors] /
 * [TurboDarkColors], the rounder [TurboShapes], the emphasized [TurboTypography],
 * and the expressive (springy, overshooting) [MotionScheme]. Everything that
 * floats over the map flips with the theme; the map raster itself does not.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun TurboTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    // MaterialExpressiveTheme defaults motionScheme to the expressive (springy,
    // overshooting) scheme — exactly what we want — so we don't pass it explicitly.
    MaterialExpressiveTheme(
        colorScheme = if (darkTheme) TurboDarkColors else TurboLightColors,
        shapes = TurboShapes,
        typography = TurboTypography,
        content = content,
    )
}
