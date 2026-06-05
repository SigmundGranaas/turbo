package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

/**
 * Turbo's warm-rust Material 3 ColorScheme, taken verbatim from the
 * `Turbo · Material 3 Expressive` design bundle (M3EKit.jsx `LIGHT`/`DARK`).
 * It reads like a leather-bound trail map: warm whites, espresso text,
 * terracotta accents, sand/cream containers.
 */

// ---- Light ----
private val LightPrimary = Color(0xFF8F4C38)
private val LightOnPrimary = Color(0xFFFFFFFF)
private val LightPrimaryContainer = Color(0xFFFFDBD1)
private val LightOnPrimaryContainer = Color(0xFF3A0F02)
private val LightSecondary = Color(0xFF77574E)
private val LightOnSecondary = Color(0xFFFFFFFF)
private val LightSecondaryContainer = Color(0xFFFFDBD1)
private val LightOnSecondaryContainer = Color(0xFF2C150E)
private val LightTertiary = Color(0xFF6C5D2F)
private val LightOnTertiary = Color(0xFFFFFFFF)
private val LightTertiaryContainer = Color(0xFFF7E2A8)
private val LightOnTertiaryContainer = Color(0xFF221B00)
private val LightError = Color(0xFFBA1A1A)
private val LightOnError = Color(0xFFFFFFFF)
private val LightErrorContainer = Color(0xFFFFDAD6)
private val LightOnErrorContainer = Color(0xFF410002)
private val LightSurface = Color(0xFFFFF8F6)
private val LightOnSurface = Color(0xFF231917)
private val LightOnSurfaceVariant = Color(0xFF53433F)
private val LightSurfaceVariant = Color(0xFFF4DDD7)
private val LightOutline = Color(0xFF85736E)
private val LightOutlineVariant = Color(0xFFD8C2BC)
private val LightSurfaceContainerLowest = Color(0xFFFFFFFF)
private val LightSurfaceContainerLow = Color(0xFFFFF1ED)
private val LightSurfaceContainer = Color(0xFFFCEAE5)
private val LightSurfaceContainerHigh = Color(0xFFF7E4E0)
private val LightSurfaceContainerHighest = Color(0xFFF1DFDA)
private val LightSurfaceBright = Color(0xFFFFF8F6)
private val LightSurfaceDim = Color(0xFFE8D6D2)
private val LightInverseSurface = Color(0xFF392E2B)
private val LightInverseOnSurface = Color(0xFFFFEDE7)
private val LightInversePrimary = Color(0xFFFFB5A0)

// ---- Dark ----
private val DarkPrimary = Color(0xFFFFB5A0)
private val DarkOnPrimary = Color(0xFF561F0F)
private val DarkPrimaryContainer = Color(0xFF723523)
private val DarkOnPrimaryContainer = Color(0xFFFFDBD1)
private val DarkSecondary = Color(0xFFE7BDB2)
private val DarkOnSecondary = Color(0xFF442A22)
private val DarkSecondaryContainer = Color(0xFF5D4037)
private val DarkOnSecondaryContainer = Color(0xFFFFDBD1)
private val DarkTertiary = Color(0xFFDAC68D)
private val DarkOnTertiary = Color(0xFF3B2F05)
private val DarkTertiaryContainer = Color(0xFF534619)
private val DarkOnTertiaryContainer = Color(0xFFF7E2A8)
private val DarkError = Color(0xFFFFB4AB)
private val DarkOnError = Color(0xFF690005)
private val DarkErrorContainer = Color(0xFF93000A)
private val DarkOnErrorContainer = Color(0xFFFFDAD6)
private val DarkSurface = Color(0xFF1A110F)
private val DarkOnSurface = Color(0xFFF1DFDA)
private val DarkOnSurfaceVariant = Color(0xFFD8C2BC)
private val DarkSurfaceVariant = Color(0xFF53433F)
private val DarkOutline = Color(0xFFA08C87)
private val DarkOutlineVariant = Color(0xFF53433F)
private val DarkSurfaceContainerLowest = Color(0xFF140C0A)
private val DarkSurfaceContainerLow = Color(0xFF231917)
private val DarkSurfaceContainer = Color(0xFF271D1B)
private val DarkSurfaceContainerHigh = Color(0xFF322825)
private val DarkSurfaceContainerHighest = Color(0xFF3D322F)
private val DarkSurfaceBright = Color(0xFF423734)
private val DarkSurfaceDim = Color(0xFF1A110F)
private val DarkInverseSurface = Color(0xFFF1DFDA)
private val DarkInverseOnSurface = Color(0xFF392E2B)
private val DarkInversePrimary = Color(0xFF8F4C38)

val TurboLightColors = lightColorScheme(
    primary = LightPrimary,
    onPrimary = LightOnPrimary,
    primaryContainer = LightPrimaryContainer,
    onPrimaryContainer = LightOnPrimaryContainer,
    secondary = LightSecondary,
    onSecondary = LightOnSecondary,
    secondaryContainer = LightSecondaryContainer,
    onSecondaryContainer = LightOnSecondaryContainer,
    tertiary = LightTertiary,
    onTertiary = LightOnTertiary,
    tertiaryContainer = LightTertiaryContainer,
    onTertiaryContainer = LightOnTertiaryContainer,
    error = LightError,
    onError = LightOnError,
    errorContainer = LightErrorContainer,
    onErrorContainer = LightOnErrorContainer,
    surface = LightSurface,
    onSurface = LightOnSurface,
    onSurfaceVariant = LightOnSurfaceVariant,
    surfaceVariant = LightSurfaceVariant,
    outline = LightOutline,
    outlineVariant = LightOutlineVariant,
    surfaceContainerLowest = LightSurfaceContainerLowest,
    surfaceContainerLow = LightSurfaceContainerLow,
    surfaceContainer = LightSurfaceContainer,
    surfaceContainerHigh = LightSurfaceContainerHigh,
    surfaceContainerHighest = LightSurfaceContainerHighest,
    surfaceBright = LightSurfaceBright,
    surfaceDim = LightSurfaceDim,
    inverseSurface = LightInverseSurface,
    inverseOnSurface = LightInverseOnSurface,
    inversePrimary = LightInversePrimary,
    surfaceTint = LightPrimary,
)

val TurboDarkColors = darkColorScheme(
    primary = DarkPrimary,
    onPrimary = DarkOnPrimary,
    primaryContainer = DarkPrimaryContainer,
    onPrimaryContainer = DarkOnPrimaryContainer,
    secondary = DarkSecondary,
    onSecondary = DarkOnSecondary,
    secondaryContainer = DarkSecondaryContainer,
    onSecondaryContainer = DarkOnSecondaryContainer,
    tertiary = DarkTertiary,
    onTertiary = DarkOnTertiary,
    tertiaryContainer = DarkTertiaryContainer,
    onTertiaryContainer = DarkOnTertiaryContainer,
    error = DarkError,
    onError = DarkOnError,
    errorContainer = DarkErrorContainer,
    onErrorContainer = DarkOnErrorContainer,
    surface = DarkSurface,
    onSurface = DarkOnSurface,
    onSurfaceVariant = DarkOnSurfaceVariant,
    surfaceVariant = DarkSurfaceVariant,
    outline = DarkOutline,
    outlineVariant = DarkOutlineVariant,
    surfaceContainerLowest = DarkSurfaceContainerLowest,
    surfaceContainerLow = DarkSurfaceContainerLow,
    surfaceContainer = DarkSurfaceContainer,
    surfaceContainerHigh = DarkSurfaceContainerHigh,
    surfaceContainerHighest = DarkSurfaceContainerHighest,
    surfaceBright = DarkSurfaceBright,
    surfaceDim = DarkSurfaceDim,
    inverseSurface = DarkInverseSurface,
    inverseOnSurface = DarkInverseOnSurface,
    inversePrimary = DarkInversePrimary,
    surfaceTint = DarkPrimary,
)

/**
 * Path palette for user-drawn paths / activity tints — from
 * `lib/features/saved_paths/models/path_style.dart`. Saturated waypoint-flag
 * colours, basemap-independent so they read on the always-light topo map.
 */
object PathPalette {
    val Blue = Color(0xFF1976D2)
    val Red = Color(0xFFD32F2F)
    val Green = Color(0xFF388E3C)
    val Orange = Color(0xFFF57C00)
    val Purple = Color(0xFF7B1FA2)
    val Teal = Color(0xFF00897B)
    val Pink = Color(0xFFC2185B)
    val all = listOf(Blue, Red, Green, Orange, Purple, Teal)
}

/** Avalanche danger ramp 1–5 (green → very-dark-red), fixed regardless of theme. */
object DangerColors {
    val all = listOf(
        Color(0xFF5BBF6A), // 1 Low
        Color(0xFFF7D23E), // 2 Moderate
        Color(0xFFF59E2E), // 3 Considerable
        Color(0xFFE0432B), // 4 High
        Color(0xFF7A1D12), // 5 Very High
    )
}
