package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.semantics.Role

/**
 * Expressive press feedback. While [interactionSource] is pressed, scale down to
 * [pressedScale] and spring back on release using the theme's expressive spatial
 * spring (slight overshoot = the M3-Expressive "pop"). Share the SAME
 * [interactionSource] with the element's `clickable`/`Surface(onClick=)` so the
 * scale tracks the same press as the ripple — or just use [pressScaleClickable].
 */
@Composable
fun Modifier.pressScale(
    interactionSource: MutableInteractionSource,
    pressedScale: Float = 0.94f,
): Modifier {
    val pressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed) pressedScale else 1f,
        animationSpec = MaterialTheme.motionScheme.fastSpatialSpec(),
        label = "pressScale",
    )
    return this.scale(scale)
}

/**
 * A `clickable` that also springs ([pressScale]) on press — one shared
 * [MutableInteractionSource] drives both the ripple and the scale, so a custom
 * `Surface`/`Box` gets real M3-Expressive press motion in one modifier.
 */
@Composable
fun Modifier.pressScaleClickable(
    onClick: () -> Unit,
    onClickLabel: String? = null,
    role: Role? = null,
    pressedScale: Float = 0.94f,
): Modifier {
    val interactionSource = remember { MutableInteractionSource() }
    return this
        .pressScale(interactionSource, pressedScale)
        .clickable(
            interactionSource = interactionSource,
            indication = LocalIndication.current,
            onClickLabel = onClickLabel,
            role = role,
            onClick = onClick,
        )
}
