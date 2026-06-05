package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

/**
 * The Expressive shape scale from M3EKit (`SHAPE`): rounder than baseline M3.
 * xs 8 · s 12 · m 16 · l 20 · xl 28 · xxl 36.
 */
val TurboShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

object TurboRadius {
    val xs = 8.dp
    val s = 12.dp
    val m = 16.dp
    val l = 20.dp
    val xl = 28.dp
    val xxl = 36.dp
}

/** Expressive container shapes that aren't part of the standard [Shapes] set. */
val ExtraExtraLargeShape = RoundedCornerShape(TurboRadius.xxl)
