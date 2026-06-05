package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * Material 3 type ramp (Roboto), with the Expressive twist from the design:
 * emphasized weights do the work — hero/display & headlines lean bold, section
 * eyebrows are 700 uppercase, buttons are 600 Title Case. Roboto ships as the
 * Android system font (a variable face on recent OSes), so no font download.
 */
private val Sans = FontFamily.Default

val TurboTypography = Typography(
    displayLarge = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W800, fontSize = 57.sp, lineHeight = 60.sp, letterSpacing = (-0.25).sp),
    displayMedium = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W800, fontSize = 45.sp, lineHeight = 52.sp),
    displaySmall = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W800, fontSize = 36.sp, lineHeight = 44.sp),

    headlineLarge = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W800, fontSize = 32.sp, lineHeight = 40.sp, letterSpacing = (-0.5).sp),
    headlineMedium = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W700, fontSize = 28.sp, lineHeight = 34.sp, letterSpacing = (-0.4).sp),
    headlineSmall = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W700, fontSize = 24.sp, lineHeight = 30.sp, letterSpacing = (-0.2).sp),

    titleLarge = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W700, fontSize = 22.sp, lineHeight = 28.sp),
    titleMedium = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W600, fontSize = 16.sp, lineHeight = 24.sp, letterSpacing = 0.15.sp),
    titleSmall = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W600, fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.1.sp),

    bodyLarge = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W400, fontSize = 16.sp, lineHeight = 24.sp, letterSpacing = 0.15.sp),
    bodyMedium = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W400, fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.25.sp),
    bodySmall = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W400, fontSize = 12.sp, lineHeight = 16.sp, letterSpacing = 0.4.sp),

    labelLarge = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W600, fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.1.sp),
    labelMedium = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W600, fontSize = 12.sp, lineHeight = 16.sp, letterSpacing = 0.5.sp),
    labelSmall = TextStyle(fontFamily = Sans, fontWeight = FontWeight.W700, fontSize = 11.sp, lineHeight = 16.sp, letterSpacing = 0.1.em),
)
