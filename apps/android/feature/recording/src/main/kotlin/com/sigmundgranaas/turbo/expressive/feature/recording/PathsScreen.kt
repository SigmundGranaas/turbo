package com.sigmundgranaas.turbo.expressive.feature.recording

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
import androidx.compose.material.icons.automirrored.rounded.TrendingDown
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Timer
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
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.foundation.Canvas
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import android.content.ClipData
import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import java.io.File
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.StatRow
import com.sigmundgranaas.turbo.expressive.ui.components.StatTile
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlin.math.roundToInt

private fun SavedPath.distanceKm(): String = "%.1f km".format(path.distanceM / 1000.0)

private fun SavedPath.ascentText(): String = "${(path.ascentM ?: 0.0).roundToInt()} m"

private fun SavedPath.descentText(): String = "${(path.descentM ?: 0.0).roundToInt()} m"

private fun SavedPath.hasElevation(): Boolean = (path.elevations?.count { it != null } ?: 0) >= 2

private fun SavedPath.durationText(): String {
    val secs = path.movingTimeSeconds ?: return "—"
    val mins = secs / 60
    return if (mins >= 60) "${mins / 60}h ${mins % 60}m" else "$mins min"
}

/** Saved tracks list, backed by the path repository (recorded tracks). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PathsListScreen(
    onBack: () -> Unit,
    onOpen: (String) -> Unit,
    viewModel: PathsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val paths by viewModel.paths.collectAsStateWithLifecycle()
    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text("Paths", style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back") } },
            )
        },
    ) { pad ->
        if (paths.isEmpty()) {
            Column(Modifier.fillMaxSize().padding(pad).padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(72.dp))
                Icon(Icons.Rounded.Route, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(40.dp))
                Spacer(Modifier.height(12.dp))
                Text("No saved tracks yet", style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
                Text("Record a track from the drawer → Record Track.", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }
        } else {
            LazyColumn(
                Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item { Spacer(Modifier.height(4.dp)); SectionLabel("Saved · ${paths.size}") }
                items(paths.size) { i ->
                    val p = paths[i]
                    PathCard(p) { onOpen(p.id) }
                }
                item { Spacer(Modifier.height(24.dp)) }
            }
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
                Text("${path.distanceKm()} · ${path.durationText()}", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }
        if (path.path.points.size > 1) {
            Spacer(Modifier.height(12.dp))
            RouteSketch(path.path.points, cs.primary, Modifier.fillMaxWidth().height(64.dp))
        }
    }
}

/** Path detail: route sketch, real stats, delete. */
@Composable
fun PathDetailScreen(
    pathId: String,
    onBack: () -> Unit,
    viewModel: PathsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val paths by viewModel.paths.collectAsStateWithLifecycle()
    val path = paths.firstOrNull { it.id == pathId }

    Column(Modifier.fillMaxSize().background(cs.surface).statusBarsPadding().navigationBarsPadding()) {
        Box(Modifier.fillMaxWidth().height(240.dp).background(Brush.verticalGradient(listOf(cs.surfaceVariant, cs.surfaceContainerHigh)))) {
            if (path != null && path.path.points.size > 1) {
                RouteSketch(path.path.points, cs.primary, Modifier.fillMaxSize().padding(24.dp))
            }
            IconButton(onClick = onBack, modifier = Modifier.padding(6.dp)) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back", tint = cs.onSurface)
            }
        }
        if (path == null) {
            Column(Modifier.padding(24.dp)) {
                Text("Track not found", style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
            }
            return@Column
        }
        val context = LocalContext.current
        Column(Modifier.padding(16.dp)) {
            Text(path.name, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Spacer(Modifier.height(14.dp))
            StatRow {
                StatTile(path.distanceKm(), "Distance", Modifier.weight(1f), Icons.Rounded.Route)
                StatTile(path.ascentText(), "Ascent", Modifier.weight(1f), Icons.Rounded.Terrain)
                StatTile(path.durationText(), "Time", Modifier.weight(1f), Icons.Rounded.Timer)
            }

            if (path.hasElevation()) {
                Spacer(Modifier.height(16.dp))
                ElevationCard(path)
            }

            Spacer(Modifier.height(16.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                FilledIconButton(
                    onClick = { shareGpx(context, path) },
                    modifier = Modifier.size(54.dp),
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer),
                ) { Icon(Icons.Rounded.IosShare, "Export GPX") }
                FilledIconButton(
                    onClick = { viewModel.delete(path.id); onBack() },
                    modifier = Modifier.size(54.dp),
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.errorContainer, contentColor = cs.onErrorContainer),
                ) { Icon(Icons.Rounded.DeleteOutline, "Delete") }
            }
        }
    }
}

/** Elevation profile card: ascent/descent summary + a filled distance-vs-height chart. */
@Composable
private fun ElevationCard(path: SavedPath) {
    val cs = MaterialTheme.colorScheme
    TurboCard(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SectionLabel("Elevation")
            Spacer(Modifier.weight(1f))
            Icon(Icons.Rounded.Terrain, null, tint = cs.primary, modifier = Modifier.size(16.dp))
            Text(" ${path.ascentText()}", style = MaterialTheme.typography.labelLarge, color = cs.onSurface)
            Spacer(Modifier.size(10.dp))
            Icon(Icons.AutoMirrored.Rounded.TrendingDown, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(16.dp))
            Text(" ${path.descentText()}", style = MaterialTheme.typography.labelLarge, color = cs.onSurfaceVariant)
        }
        Spacer(Modifier.height(12.dp))
        ElevationProfile(
            elevations = path.path.elevations.orEmpty(),
            line = cs.primary,
            fill = cs.primary.copy(alpha = 0.18f),
            modifier = Modifier.fillMaxWidth().height(96.dp),
        )
    }
}

/** Filled elevation curve over point index; null altitudes are bridged across. */
@Composable
private fun ElevationProfile(elevations: List<Double?>, line: Color, fill: Color, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val present = elevations.withIndex().filter { it.value != null }
        if (present.size < 2) return@Canvas
        val minE = present.minOf { it.value!! }
        val maxE = present.maxOf { it.value!! }
        val range = (maxE - minE).coerceAtLeast(1.0)
        val lastIdx = (elevations.size - 1).coerceAtLeast(1)
        fun px(i: Int) = i.toFloat() / lastIdx * size.width
        fun py(e: Double) = (1f - ((e - minE) / range).toFloat()) * size.height

        val stroke = Path()
        present.forEachIndexed { order, (i, e) ->
            val x = px(i); val y = py(e!!)
            if (order == 0) stroke.moveTo(x, y) else stroke.lineTo(x, y)
        }
        val area = Path().apply {
            addPath(stroke)
            lineTo(px(present.last().index), size.height)
            lineTo(px(present.first().index), size.height)
            close()
        }
        drawPath(area, fill)
        drawPath(stroke, line, style = Stroke(width = 4f))
    }
}

/** Write the track to a cache .gpx and fire a share chooser via FileProvider. */
private fun shareGpx(context: Context, path: SavedPath) {
    val dir = File(context.cacheDir, "gpx").apply { mkdirs() }
    val file = File(dir, gpxFileName(path.name))
    file.writeText(pathToGpx(path))
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    val send = Intent(Intent.ACTION_SEND).apply {
        type = "application/gpx+xml"
        putExtra(Intent.EXTRA_STREAM, uri)
        // ClipData grants the read permission to the chooser preview + target alike.
        clipData = ClipData.newRawUri(path.name, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(send, "Share GPX").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Normalised polyline sketch of a path's points into the given box. */
@Composable
private fun RouteSketch(points: List<LatLng>, color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        if (points.size < 2) return@Canvas
        val lats = points.map { it.lat }
        val lngs = points.map { it.lng }
        val minLat = lats.min(); val maxLat = lats.max()
        val minLng = lngs.min(); val maxLng = lngs.max()
        fun proj(p: LatLng): Offset {
            val x = ((p.lng - minLng) / (maxLng - minLng + 1e-9)) * size.width
            val y = (1 - (p.lat - minLat) / (maxLat - minLat + 1e-9)) * size.height
            return Offset(x.toFloat(), y.toFloat())
        }
        val line = Path().apply {
            moveTo(proj(points.first()).x, proj(points.first()).y)
            points.drop(1).forEach { lineTo(proj(it).x, proj(it).y) }
        }
        drawPath(line, color = color, style = Stroke(width = 5f))
    }
}
