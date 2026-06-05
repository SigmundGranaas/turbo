package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.googlefonts.Font
import androidx.compose.ui.text.googlefonts.GoogleFont
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.sigmundgranaas.turbo.expressive.core.designsystem.R

/**
 * Material 3 type ramp (Roboto Flex), with the Expressive twist from the design:
 * emphasized weights do the work — hero/display & headlines lean bold, section
 * eyebrows are 700 uppercase, buttons are 600 Title Case. Roboto Flex is pulled
 * as a downloadable Google Font (variable face); if the provider is unavailable
 * Compose falls back to the bundled system sans, so text always renders.
 */
private val provider = GoogleFont.Provider(
    providerAuthority = "com.google.android.gms.fonts",
    providerPackage = "com.google.android.gms",
    certificates = R.array.com_google_android_gms_fonts_certs,
)

private val RobotoFlex = GoogleFont("Roboto Flex")

private val Sans = FontFamily(
    Font(googleFont = RobotoFlex, fontProvider = provider, weight = FontWeight.W400),
    Font(googleFont = RobotoFlex, fontProvider = provider, weight = FontWeight.W500),
    Font(googleFont = RobotoFlex, fontProvider = provider, weight = FontWeight.W600),
    Font(googleFont = RobotoFlex, fontProvider = provider, weight = FontWeight.W700),
    Font(googleFont = RobotoFlex, fontProvider = provider, weight = FontWeight.W800),
)

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
