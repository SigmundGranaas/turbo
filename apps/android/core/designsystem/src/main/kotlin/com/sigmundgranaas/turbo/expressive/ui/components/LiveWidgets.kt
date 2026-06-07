package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.sin

/**
 * The signature animated "live GPS" motif: a wavy line that pans horizontally, a
 * Compose translation of the design's repeating SVG wave. Conveys that fixes are
 * streaming in. Purely decorative (no semantics).
 */
@Composable
fun WaveStrip(
    color: Color,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 18.dp,
    animate: Boolean = true,
) {
    val transition = rememberInfiniteTransition(label = "wave")
    val phase = if (animate) {
        transition.animateFloat(
            initialValue = 0f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(1_100, easing = LinearEasing), RepeatMode.Restart),
            label = "wavePhase",
        ).value
    } else {
        0f
    }
    Canvas(modifier.fillMaxWidth().height(height).testTag("waveStrip")) {
        val h = size.height
        val mid = h / 2f
        val amp = h / 2.6f
        val wavelength = with(this) { 44.dp.toPx() }
        val shift = phase * wavelength
        val path = Path()
        var x = -wavelength + shift
        path.moveTo(x, mid)
        val stepPx = 4f
        while (x <= size.width + wavelength) {
            val y = mid + amp * sin((x - shift) / wavelength * 2f * Math.PI.toFloat())
            path.lineTo(x, y)
            x += stepPx
        }
        drawPath(path, color, style = Stroke(width = 3.dp.toPx(), cap = StrokeCap.Round))
    }
}

/** The vibrant tonal palettes a [LiveMetricTile] can take, mirroring the design's bento. */
enum class MetricTone { Neutral, Primary, Secondary, Tertiary, Green, Red }

private data class TonePalette(val bg: Color, val fg: Color, val sub: Color, val chip: Color, val chipFg: Color)

@Composable
private fun MetricTone.palette(): TonePalette {
    val cs = MaterialTheme.colorScheme
    val dark = cs.surface.luminance() < 0.5f
    val green = Color(0xFF2E7D32)
    val red = Color(0xFFC0392B)
    val greenFg = if (dark) Color(0xFF7FD99A) else Color(0xFF1C5A26)
    val redFg = if (dark) Color(0xFFFF9E92) else Color(0xFF8F231A)
    return when (this) {
        MetricTone.Neutral -> TonePalette(cs.surfaceContainerHigh, cs.onSurface, cs.onSurfaceVariant, cs.primary, cs.onPrimary)
        MetricTone.Primary -> TonePalette(cs.primaryContainer, cs.onPrimaryContainer, cs.onPrimaryContainer.copy(alpha = .75f), cs.primary, cs.onPrimary)
        MetricTone.Secondary -> TonePalette(cs.secondaryContainer, cs.onSecondaryContainer, cs.onSecondaryContainer.copy(alpha = .75f), cs.secondary, cs.onSecondary)
        MetricTone.Tertiary -> TonePalette(cs.tertiaryContainer, cs.onTertiaryContainer, cs.onTertiaryContainer.copy(alpha = .75f), cs.tertiary, cs.onTertiary)
        MetricTone.Green -> TonePalette(green.copy(alpha = if (dark) .26f else .15f), greenFg, greenFg.copy(alpha = .8f), green, Color.White)
        MetricTone.Red -> TonePalette(red.copy(alpha = if (dark) .26f else .14f), redFg, redFg.copy(alpha = .8f), red, Color.White)
    }
}

/**
 * A color-drenched bento metric tile: a scallop [Cookie] icon badge above an
 * uppercase label, with a big value + unit beneath. The vibrant tonal building
 * block of the live recording / following sheets.
 */
@Composable
fun LiveMetricTile(
    icon: ImageVector,
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    unit: String? = null,
    tone: MetricTone = MetricTone.Neutral,
) {
    val p = tone.palette()
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(TurboRadius.l))
            .background(p.bg)
            .padding(horizontal = 14.dp, vertical = 13.dp)
            .clearAndSetSemantics { contentDescription = "$label: $value${unit?.let { " $it" } ?: ""}" },
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Cookie(size = 30.dp, fill = p.chip) {
                Icon(icon, null, tint = p.chipFg, modifier = Modifier.size(17.dp))
            }
            Spacer(Modifier.width(9.dp))
            Text(
                label.uppercase(),
                style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.W700, letterSpacing = 0.5.sp),
                color = p.sub,
            )
        }
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                value,
                style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W800),
                color = p.fg,
            )
            if (unit != null) {
                Spacer(Modifier.width(4.dp))
                Text(
                    unit,
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
                    color = p.sub,
                    modifier = Modifier.padding(bottom = 3.dp),
                )
            }
        }
    }
}

/**
 * A compact elevation profile: a gradient-filled area under the elevation curve
 * with the current position marked. [elevations] is the per-point altitude
 * series; [progress] (0..1) places the marker. Header carries a label + value.
 */
@Composable
fun LiveElevationSpark(
    elevations: List<Double>,
    progress: Float,
    label: String,
    value: String,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.l))
            .background(cs.surfaceContainerHigh)
            .padding(horizontal = 16.dp, vertical = 13.dp)
            .testTag("elevationSpark"),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Cookie(size = 30.dp, fill = cs.tertiary) {
                Icon(
                    androidx.compose.material.icons.Icons.Rounded.Terrain, null,
                    tint = cs.onTertiary, modifier = Modifier.size(17.dp),
                )
            }
            Spacer(Modifier.width(9.dp))
            Text(label.uppercase(), style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.W700), color = cs.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            Text(value, style = MaterialTheme.typography.labelLarge, color = cs.onSurface)
        }
        Spacer(Modifier.height(10.dp))
        val line = cs.primary
        Box(Modifier.fillMaxWidth().height(62.dp)) {
            Canvas(Modifier.fillMaxWidth().height(62.dp)) {
                if (elevations.size < 2) return@Canvas
                val min = elevations.min()
                val max = elevations.max()
                val span = (max - min).takeIf { it > 1e-6 } ?: 1.0
                val n = elevations.size
                fun px(i: Int) = size.width * i / (n - 1).toFloat()
                fun py(v: Double) = size.height * (1f - ((v - min) / span).toFloat()) * 0.86f + size.height * 0.07f
                val area = Path().apply {
                    moveTo(0f, size.height)
                    lineTo(0f, py(elevations[0]))
                    for (i in 1 until n) lineTo(px(i), py(elevations[i]))
                    lineTo(size.width, size.height)
                    close()
                }
                drawPath(area, Brush.verticalGradient(listOf(line.copy(alpha = .42f), line.copy(alpha = 0f))))
                val stroke = Path().apply {
                    moveTo(0f, py(elevations[0]))
                    for (i in 1 until n) lineTo(px(i), py(elevations[i]))
                }
                drawPath(stroke, line, style = Stroke(width = 2.6.dp.toPx(), cap = StrokeCap.Round))
                val markerX = size.width * progress.coerceIn(0f, 1f)
                drawLine(cs.onSurfaceVariant, Offset(markerX, 0f), Offset(markerX, size.height), strokeWidth = 1.4.dp.toPx())
            }
        }
    }
}
