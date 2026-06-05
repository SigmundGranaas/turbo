package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp

/**
 * The single map-marker type used across the whole app — a clean terracotta
 * teardrop (rounded square rotated 45° with a sharp bottom point), a white
 * casing ring, a centred filled glyph, and a soft grounded shadow ellipse so
 * it sits on the map rather than floats. Matches M3EKit `MarkerPin`.
 *
 * Anchored bottom-centre: place the composable so its bottom edge is at the
 * marker's screen position.
 */
@Composable
fun MarkerPin(
    icon: ImageVector,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    color: Color = MaterialTheme.colorScheme.primary,
) {
    val ring = MaterialTheme.colorScheme.surface
    val box = if (selected) 42.dp else 33.dp
    val glyph = if (selected) 20.dp else 15.dp
    val borderW = if (selected) 3.dp else 2.5.dp
    // 50% 50% 50% 2px → after a 45° rotation the bottom-left corner becomes the tip.
    val teardrop = RoundedCornerShape(
        topStartPercent = 50, topEndPercent = 50, bottomEndPercent = 50, bottomStartPercent = 0,
    )

    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = modifier) {
        Box(
            modifier = Modifier
                .size(box)
                .graphicsLayer { shadowElevation = 6f; shape = teardrop; clip = false }
                .rotate(45f)
                .clip(teardrop)
                .background(color)
                .border(borderW, ring, teardrop),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier
                    .rotate(-45f)
                    .size(glyph),
            )
        }
        // Grounded shadow ellipse just below the tip.
        Box(
            modifier = Modifier
                .padding(top = 1.dp)
                .size(width = if (selected) 14.dp else 10.dp, height = if (selected) 4.dp else 3.dp)
                .clip(RoundedCornerShape(50))
                .background(Color(0x38281408)),
        )
    }
}
