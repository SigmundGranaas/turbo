package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * The Expressive scalloped "cookie" container that carries activity glyphs —
 * the soft petal silhouette from M3EKit's `cookiePath` (8 lobes, 8% depth,
 * deliberately gentle rather than spiky). A [Shape] so any content can be
 * clipped/backed by it.
 */
class CookieShape(
    private val lobes: Int = 8,
    private val depth: Float = 0.08f,
    private val steps: Int = 220,
) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline {
        val r = minOf(size.width, size.height) / 2f
        val cx = size.width / 2f
        val cy = size.height / 2f
        val amp = r * depth
        val mid = r - amp
        val path = Path()
        for (i in 0..steps) {
            val t = i.toFloat() / steps * (PI * 2).toFloat()
            val rad = mid + amp * cos(lobes * t)
            val x = cx + rad * cos(t - (PI / 2).toFloat())
            val y = cy + rad * sin(t - (PI / 2).toFloat())
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        path.close()
        return Outline.Generic(path)
    }
}

/** A cookie-shaped container filled with [fill], centring [content]. */
@Composable
fun Cookie(
    size: Dp,
    fill: Color,
    modifier: Modifier = Modifier,
    lobes: Int = 8,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = modifier
            .size(size)
            .clip(CookieShape(lobes = lobes))
            .background(fill),
        contentAlignment = Alignment.Center,
        content = { content() },
    )
}
