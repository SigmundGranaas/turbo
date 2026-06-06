package com.sigmundgranaas.turbo.expressive.ui.layout

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Three coarse width buckets, mirroring the Material window-size-class breakpoints
 * (we derive them from [LocalConfiguration] rather than pulling the
 * material3-window-size-class artifact, which isn't on this alpha track).
 *
 * - [Compact]  < 600 dp — phones in portrait
 * - [Medium]   600–839 dp — large phones landscape / small tablets
 * - [Expanded] ≥ 840 dp — tablets / desktop / foldables unfolded
 */
enum class WindowWidthClass { Compact, Medium, Expanded }

/** The current [WindowWidthClass] from the active configuration. */
@Composable
@ReadOnlyComposable
fun rememberWindowWidthClass(): WindowWidthClass {
    val widthDp = LocalConfiguration.current.screenWidthDp
    return when {
        widthDp < 600 -> WindowWidthClass.Compact
        widthDp < 840 -> WindowWidthClass.Medium
        else -> WindowWidthClass.Expanded
    }
}

/** True once there's room for side-by-side / multi-column layouts. */
@Composable
@ReadOnlyComposable
fun isExpandedWidth(): Boolean = rememberWindowWidthClass() == WindowWidthClass.Expanded

/**
 * Caps a scrolling content column to [max] and centres it. On a phone the column
 * just fills the width; on a tablet/landscape it stops sprawling into
 * unreadably-long lines and sits centred. Apply to the Column/LazyColumn that
 * holds a screen's body (NOT to the top app bar, which should span full width).
 */
fun Modifier.responsiveContentWidth(max: Dp = 640.dp): Modifier =
    this
        .fillMaxWidth()
        .wrapContentWidth(Alignment.CenterHorizontally)
        .widthIn(max = max)

/** Adaptive grid column count for list screens: 1 on phones, 2 once expanded. */
@Composable
@ReadOnlyComposable
fun adaptiveColumns(): Int = if (isExpandedWidth()) 2 else 1
