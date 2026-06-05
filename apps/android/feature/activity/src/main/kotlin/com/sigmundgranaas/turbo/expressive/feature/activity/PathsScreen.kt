package com.sigmundgranaas.turbo.expressive.feature.activity

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SampleData
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.StatRow
import com.sigmundgranaas.turbo.expressive.ui.components.StatTile
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.roundToInt

/** A saved path/track for the paths list. */
data class SavedPath(
    val id: String,
    val name: String,
    val place: String,
    val points: List<LatLng>,
    val elevations: List<Double?>,
)

private val sampleElevations = listOf(12.0, 60.0, 140.0, 421.0, 380.0, 250.0, 120.0, 14.0)

private val samplePaths = listOf(
    SavedPath("p-storsteinen", "Storsteinen Loop", "Tromsøya · Troms", SampleData.storsteinenLoop, sampleElevations),
    SavedPath("p-tromsdalstind", "Tromsdalstind", "Tromsdalen · Troms", SampleData.storsteinenLoop, sampleElevations.map { it * 2.4 }),
)

private fun SavedPath.distanceKm(): String =
    "%.1f km".format(GeoMetrics.pathLengthMeters(points) / 1000.0)

private fun SavedPath.ascent(): String {
    val (asc, _) = GeoMetrics.gainLoss(elevations)
    return "${(asc ?: 0.0).roundToInt()} m"
}

private fun SavedPath.etaText(): String {
    val secs = GeoMetrics.pathLengthMeters(points) / 1.2 // ~1.2 m/s walking
    val mins = (secs / 60).roundToInt()
    return if (mins >= 60) "${mins / 60}h ${mins % 60}m" else "$mins min"
}

/** Paths list: each saved track as a card with a route sparkline + stat strip. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PathsListScreen(onBack: () -> Unit, onOpen: (String) -> Unit) {
    val cs = MaterialTheme.colorScheme
    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text("Paths", style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back") } },
            )
        },
    ) { pad ->
        LazyColumn(
            Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { Spacer(Modifier.height(4.dp)); SectionLabel("Saved tracks · ${samplePaths.size}") }
            items(samplePaths.size) { i ->
                val p = samplePaths[i]
                PathCard(p) { onOpen(p.id) }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
}

@Composable
private fun PathCard(path: SavedPath, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .clickable(onClick = onClick).padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(44.dp).clip(RoundedCornerShape(TurboRadius.m)).background(cs.primaryContainer), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.Route, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.size(12.dp))
            Column(Modifier.weight(1f)) {
                Text(path.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(path.place, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }
        Spacer(Modifier.height(12.dp))
        ElevationSpark(cs.primary, Modifier.fillMaxWidth().height(48.dp))
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(18.dp)) {
            MiniStat(path.distanceKm(), "Distance")
            MiniStat(path.ascent(), "Ascent")
            MiniStat(path.etaText(), "Est. time")
        }
    }
}

@Composable
private fun MiniStat(value: String, label: String) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(value, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

/** Path detail: route map placeholder, stat strip, elevation profile, actions. */
@Composable
fun PathDetailScreen(pathId: String, onBack: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val path = samplePaths.firstOrNull { it.id == pathId } ?: samplePaths.first()
    Column(Modifier.fillMaxSize().background(cs.surface).statusBarsPadding().navigationBarsPadding()) {
        Box(Modifier.fillMaxWidth().height(220.dp).background(Brush.verticalGradient(listOf(cs.surfaceVariant, cs.surfaceContainerHigh)))) {
            Canvas(Modifier.fillMaxSize().padding(24.dp)) {
                val pts = path.points
                if (pts.size >= 2) {
                    val lats = pts.map { it.lat }
                    val lngs = pts.map { it.lng }
                    val minLat = lats.min(); val maxLat = lats.max()
                    val minLng = lngs.min(); val maxLng = lngs.max()
                    fun proj(p: LatLng): Offset {
                        val x = ((p.lng - minLng) / (maxLng - minLng + 1e-9)) * size.width
                        val y = (1 - (p.lat - minLat) / (maxLat - minLat + 1e-9)) * size.height
                        return Offset(x.toFloat(), y.toFloat())
                    }
                    val line = Path().apply {
                        moveTo(proj(pts.first()).x, proj(pts.first()).y)
                        pts.drop(1).forEach { lineTo(proj(it).x, proj(it).y) }
                    }
                    drawPath(line, color = cs.primary, style = Stroke(width = 5f))
                }
            }
            IconButton(onClick = onBack, modifier = Modifier.padding(6.dp)) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back", tint = cs.onSurface)
            }
        }
        Column(Modifier.padding(16.dp)) {
            Text(path.name, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Text(path.place, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            Spacer(Modifier.height(14.dp))
            StatRow {
                StatTile(path.distanceKm(), "Distance", Modifier.weight(1f), Icons.Rounded.Route)
                StatTile(path.ascent(), "Ascent", Modifier.weight(1f), Icons.Rounded.Terrain)
                StatTile(path.etaText(), "Est. time", Modifier.weight(1f), Icons.Rounded.Timer)
            }
            Spacer(Modifier.height(12.dp))
            TurboCard {
                SectionLabel("Elevation profile")
                Spacer(Modifier.height(10.dp))
                ElevationSpark(cs.primary, Modifier.fillMaxWidth().height(96.dp))
            }
            Spacer(Modifier.height(16.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Button(onClick = {}, modifier = Modifier.weight(1f).height(54.dp)) {
                    Icon(Icons.Rounded.Navigation, null, modifier = Modifier.size(20.dp)); Spacer(Modifier.size(8.dp)); Text("Follow", style = MaterialTheme.typography.titleMedium)
                }
                FilledIconButton(
                    onClick = {}, modifier = Modifier.size(54.dp),
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer),
                ) { Icon(Icons.Rounded.IosShare, "Share") }
            }
        }
    }
}

/** Soft-filled elevation sparkline driven by the sample profile. */
@Composable
private fun ElevationSpark(color: Color, modifier: Modifier = Modifier) {
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
