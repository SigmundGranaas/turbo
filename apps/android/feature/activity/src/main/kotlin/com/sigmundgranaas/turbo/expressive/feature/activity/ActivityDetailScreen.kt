package com.sigmundgranaas.turbo.expressive.feature.activity

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.AcUnit
import androidx.compose.material.icons.rounded.Bookmark
import androidx.compose.material.icons.rounded.DownhillSkiing
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Thermostat
import androidx.compose.material3.Button
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.theme.DangerColors
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@Composable
fun ActivityDetailScreen(onBack: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    var tab by remember { mutableIntStateOf(0) }
    val tabs = listOf("Snow", "Terrain", "Weather")

    Column(Modifier.fillMaxSize().background(cs.surface).verticalScroll(rememberScrollState())) {
        // App bar
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(horizontal = 6.dp, vertical = 6.dp)) {
            IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back", tint = cs.onSurface) }
            SectionLabel("Backcountry Ski", color = cs.primary, modifier = Modifier.weight(1f).padding(start = 6.dp))
            IconButton(onClick = {}) { Icon(Icons.Rounded.IosShare, "Share", tint = cs.onSurfaceVariant) }
        }

        // Title
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 20.dp).padding(bottom = 14.dp)) {
            Cookie(size = 56.dp, fill = cs.tertiaryContainer) { Icon(Icons.Rounded.DownhillSkiing, null, tint = cs.onTertiaryContainer, modifier = Modifier.size(28.dp)) }
            Spacer(Modifier.size(14.dp))
            Column {
                Text("Tamokdalen NW", style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
                Text("Tamokdalen · Troms", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }
        }

        // Verdict card
        val danger = DangerColors.all[2]
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth()
                .clip(RoundedCornerShape(TurboRadius.xl)).background(danger.copy(alpha = 0.12f))
                .border(1.dp, danger.copy(alpha = 0.4f), RoundedCornerShape(TurboRadius.xl)).padding(16.dp),
        ) {
            DangerBars(level = 3)
            Spacer(Modifier.size(14.dp))
            Column {
                Text("Considerable · Level 3", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800), color = cs.onSurface)
                Text("Wind slab on N–E aspects above 900 m. Varsom · today", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }

        Spacer(Modifier.height(14.dp))
        SingleChoiceSegmentedButtonRow(Modifier.padding(horizontal = 16.dp).fillMaxWidth()) {
            tabs.forEachIndexed { i, label ->
                SegmentedButton(
                    selected = tab == i,
                    onClick = { tab = i },
                    shape = SegmentedButtonDefaults.itemShape(i, tabs.size),
                ) { Text(label) }
            }
        }

        Spacer(Modifier.height(14.dp))
        when (tab) {
            1 -> Placeholder("Terrain", "ATES rating, slope angle shading, and the ascent/descent legs live here.")
            2 -> Placeholder("Weather", "Snow temperature, 24h snowfall, wind transport and the 6-hour strip live here.")
            else -> SnowTab()
        }

        // Description
        Text(
            "North-west couloir off Tamokdalen. Skin track follows the summer trail to 900 m, then traverses skier's left below the cornice line. Ski the apron, not the gut, in current conditions.",
            style = MaterialTheme.typography.bodyMedium,
            color = cs.onSurface,
            modifier = Modifier.padding(horizontal = 22.dp, vertical = 14.dp),
        )

        // Action bar
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(horizontal = 18.dp).padding(bottom = 24.dp)) {
            Button(onClick = {}, modifier = Modifier.weight(1f).height(54.dp)) {
                Icon(Icons.Rounded.Navigation, null, modifier = Modifier.size(20.dp)); Spacer(Modifier.size(8.dp)); Text("Navigate", style = MaterialTheme.typography.titleMedium)
            }
            FilledIconButton(
                onClick = {}, modifier = Modifier.size(54.dp),
                colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer),
            ) { Icon(Icons.Rounded.Bookmark, "Save") }
        }
    }
}

@Composable
private fun SnowTab() {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp)) {
        Card {
            SectionLabel("Danger by aspect & elevation")
            Spacer(Modifier.height(16.dp))
            AspectRose(Modifier.align(Alignment.CenterHorizontally))
        }
        Spacer(Modifier.height(12.dp))
        Card {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                SectionLabel("Route profile")
                Text("1240 m ascent", style = MaterialTheme.typography.labelLarge, color = cs.primary)
            }
            Spacer(Modifier.height(10.dp))
            ElevationProfile(cs.primary, Modifier.fillMaxWidth().height(74.dp))
        }
        Spacer(Modifier.height(12.dp))
        Card {
            SectionLabel("Snowpack")
            ListRowItem(Icons.Rounded.AcUnit, "New snow · 24h", subtitle = "settling", trailing = { Text("22 cm", style = MaterialTheme.typography.titleMedium, color = cs.onSurface) })
            ListRowItem(Icons.Rounded.Thermostat, "Freezing level", subtitle = "rising to 600 m by noon", trailing = { Text("600 m", style = MaterialTheme.typography.titleMedium, color = cs.onSurface) })
        }
    }
}

@Composable
private fun Card(content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit) {
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh).padding(18.dp),
        content = content,
    )
}

@Composable
private fun Placeholder(title: String, body: String) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.padding(horizontal = 16.dp)) {
        Card {
            SectionLabel(title)
            Spacer(Modifier.height(8.dp))
            Text(body, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
        }
    }
}

/** 5-bar avalanche danger badge (green→very-dark-red), filled to [level]. */
@Composable
private fun DangerBars(level: Int) {
    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        val inactive = MaterialTheme.colorScheme.surfaceContainerHighest
        for (n in 1..5) {
            Box(
                Modifier.size(width = 9.dp, height = (8 + n * 4).dp).clip(RoundedCornerShape(3.dp))
                    .background(if (n <= level) DangerColors.all[n - 1] else inactive),
            )
        }
    }
}

/** 8-octant aspect rose with a centre elevation label. */
@Composable
private fun AspectRose(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    val octants = listOf(2, 3, 4, 4, 3, 2, 2, 2) // danger per N,NE,E,SE,S,SW,W,NW
    val ring = cs.surfaceContainerLowest
    Box(modifier.size(150.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(150.dp)) {
            octants.forEachIndexed { i, d ->
                drawArc(
                    color = DangerColors.all[d - 1],
                    startAngle = -90f + i * 45f,
                    sweepAngle = 45f,
                    useCenter = true,
                    alpha = 0.92f,
                )
            }
            // inner hole
            val hole = size.minDimension * 0.46f
            drawCircle(color = ring, radius = hole / 2f, center = center)
        }
        Text("2200 m", style = MaterialTheme.typography.labelLarge, color = cs.onSurface)
    }
}

/** Slim elevation sparkline with a soft fill, matching the design's ElevProfile. */
@Composable
private fun ElevationProfile(color: Color, modifier: Modifier = Modifier) {
    val ys = listOf(52, 40, 44, 28, 30, 14, 20, 8, 16, 30, 26, 38, 46, 40, 52)
    Canvas(modifier) {
        val stepX = size.width / (ys.size - 1)
        fun px(i: Int) = Offset(i * stepX, ys[i] / 64f * size.height)
        val line = Path().apply {
            moveTo(px(0).x, px(0).y)
            for (i in 1 until ys.size) lineTo(px(i).x, px(i).y)
        }
        val fill = Path().apply {
            addPath(line)
            lineTo(size.width, size.height); lineTo(0f, size.height); close()
        }
        drawPath(fill, color = color.copy(alpha = 0.16f))
        drawPath(line, color = color, style = Stroke(width = 2.4.dp.toPx()))
    }
}
