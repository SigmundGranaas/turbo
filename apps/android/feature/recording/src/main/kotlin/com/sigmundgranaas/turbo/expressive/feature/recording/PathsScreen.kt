package com.sigmundgranaas.turbo.expressive.feature.recording

import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.material.icons.rounded.FileUpload
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
import androidx.compose.ui.res.stringResource
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
import com.sigmundgranaas.turbo.expressive.ui.components.ConfirmDeleteDialog
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.StatRow
import com.sigmundgranaas.turbo.expressive.ui.components.StatTile
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth
import com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.core.geo.Units

private fun SavedPath.distanceText(metric: Boolean): String = Units.distance(path.distanceM, metric)

private fun SavedPath.ascentText(metric: Boolean): String = Units.elevation(path.ascentM ?: 0.0, metric)

private fun SavedPath.descentText(metric: Boolean): String = Units.elevation(path.descentM ?: 0.0, metric)

private fun SavedPath.hasElevation(): Boolean = (path.elevations?.count { it != null } ?: 0) >= 2

private fun SavedPath.durationText(): String {
    val secs = path.movingTimeSeconds ?: return "—"
    val mins = secs / 60
    return if (mins >= 60) "${mins / 60}h ${mins % 60}m" else "$mins min"
}

/** Ordering options for the saved-tracks list. */
internal enum class PathSort(val label: String) {
    Newest("Newest"), Name("Name"), Longest("Longest")
}

/** Filter by case-insensitive name substring, then order by [sort]. Pure for testing. */
internal fun sortAndFilterPaths(paths: List<SavedPath>, query: String, sort: PathSort): List<SavedPath> {
    val q = query.trim()
    val filtered = if (q.isEmpty()) paths else paths.filter { it.name.contains(q, ignoreCase = true) }
    return when (sort) {
        PathSort.Newest -> filtered.sortedByDescending { it.path.recordedAtEpochMs ?: 0L }
        PathSort.Name -> filtered.sortedBy { it.name.lowercase() }
        PathSort.Longest -> filtered.sortedByDescending { it.path.distanceM }
    }
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
    val context = LocalContext.current
    // Open any GPX/KML/GeoJSON document, parse it, and persist the track.
    val importLauncher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val body = runCatching {
            context.contentResolver.openInputStream(uri)?.use { it.readBytes().decodeToString() }
        }.getOrNull()
        val parsed = body?.let { TrackImport.parse(it) }
        if (parsed != null) {
            viewModel.importTrack(parsed, fallbackName = displayName(context, uri) ?: context.getString(R.string.paths_imported_default))
            Toast.makeText(context, context.getString(R.string.paths_imported), Toast.LENGTH_SHORT).show()
        } else {
            Toast.makeText(context, context.getString(R.string.paths_import_failed), Toast.LENGTH_SHORT).show()
        }
    }
    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.paths_title), style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.paths_back)) } },
                actions = {
                    IconButton(onClick = {
                        importLauncher.launch(arrayOf("application/gpx+xml", "application/vnd.google-earth.kml+xml", "application/geo+json", "application/xml", "text/xml", "application/json", "*/*"))
                    }) { Icon(Icons.Rounded.FileUpload, stringResource(R.string.paths_import)) }
                },
            )
        },
    ) { pad ->
        var query by rememberSaveable { mutableStateOf("") }
        var sort by rememberSaveable { mutableStateOf(PathSort.Newest) }
        if (paths.isEmpty()) {
            EmptyState(
                icon = Icons.Rounded.Route,
                title = stringResource(R.string.paths_empty_title),
                body = stringResource(R.string.paths_empty_body),
                modifier = Modifier.fillMaxSize().padding(pad),
            )
        } else {
            val visible = remember(paths, query, sort) { sortAndFilterPaths(paths, query, sort) }
            Column(Modifier.fillMaxSize().padding(pad)) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text(stringResource(R.string.paths_search)) },
                    leadingIcon = { Icon(Icons.Rounded.Search, null) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
                Row(
                    Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    PathSort.entries.forEach { mode ->
                        val labelRes = when (mode) {
                            PathSort.Newest -> R.string.paths_sort_newest
                            PathSort.Name -> R.string.paths_sort_name
                            PathSort.Longest -> R.string.paths_sort_longest
                        }
                        FilterChip(
                            selected = mode == sort,
                            onClick = { sort = mode },
                            label = { Text(stringResource(labelRes)) },
                        )
                    }
                }
                if (visible.isEmpty()) {
                    EmptyState(
                        icon = Icons.Rounded.Search,
                        title = stringResource(R.string.paths_no_matches),
                        body = stringResource(R.string.paths_no_matches_body, query),
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    LazyColumn(
                        Modifier.fillMaxHeight().responsiveContentWidth().padding(horizontal = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        item { SectionLabel(stringResource(R.string.paths_saved_count, visible.size)) }
                        items(visible.size) { i ->
                            val p = visible[i]
                            PathCard(p) { onOpen(p.id) }
                        }
                        item { Spacer(Modifier.height(24.dp)) }
                    }
                }
            }
        }
    }
}

@Composable
private fun PathCard(path: SavedPath, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val metric = LocalMetricUnits.current
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .clickable(onClick = onClick).padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(44.dp).clip(RoundedCornerShape(TurboRadius.m)).background(cs.primaryContainer), contentAlignment = Alignment.Center) {
                Icon(
                    path.activityKind?.icon ?: Icons.Rounded.Route,
                    null,
                    tint = cs.onPrimaryContainer,
                    modifier = Modifier.size(24.dp),
                )
            }
            Spacer(Modifier.size(12.dp))
            Column(Modifier.weight(1f)) {
                Text(path.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("${path.distanceText(metric)} · ${path.durationText()}", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
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
    var showDelete by remember { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().background(cs.surface).statusBarsPadding().navigationBarsPadding()) {
        Box(Modifier.fillMaxWidth().height(240.dp).background(Brush.verticalGradient(listOf(cs.surfaceVariant, cs.surfaceContainerHigh)))) {
            if (path != null && path.path.points.size > 1) {
                RouteSketch(path.path.points, cs.primary, Modifier.fillMaxSize().padding(24.dp))
            }
            IconButton(onClick = onBack, modifier = Modifier.padding(6.dp)) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.paths_back), tint = cs.onSurface)
            }
        }
        if (path == null) {
            Column(Modifier.padding(24.dp)) {
                Text(stringResource(R.string.paths_not_found), style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
            }
            return@Column
        }
        val context = LocalContext.current
        val metric = LocalMetricUnits.current
        Column(Modifier.padding(16.dp)) {
            Text(path.name, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Spacer(Modifier.height(14.dp))
            StatRow {
                StatTile(path.distanceText(metric), stringResource(R.string.paths_distance), Modifier.weight(1f), Icons.Rounded.Route)
                StatTile(path.ascentText(metric), stringResource(R.string.paths_ascent), Modifier.weight(1f), Icons.Rounded.Terrain)
                StatTile(path.durationText(), stringResource(R.string.paths_time), Modifier.weight(1f), Icons.Rounded.Timer)
            }

            if (path.hasElevation()) {
                Spacer(Modifier.height(16.dp))
                ElevationCard(path, metric)
            }

            Spacer(Modifier.height(16.dp))
            var showExportMenu by remember { mutableStateOf(false) }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box {
                    FilledIconButton(
                        onClick = { showExportMenu = true },
                        modifier = Modifier.size(54.dp),
                        colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer),
                    ) { Icon(Icons.Rounded.IosShare, stringResource(R.string.paths_export)) }
                    DropdownMenu(expanded = showExportMenu, onDismissRequest = { showExportMenu = false }) {
                        ExportFormat.entries.forEach { format ->
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.paths_export_format, format.label)) },
                                onClick = { showExportMenu = false; shareTrack(context, path, format) },
                            )
                        }
                    }
                }
                FilledIconButton(
                    onClick = { showDelete = true },
                    modifier = Modifier.size(54.dp),
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = cs.errorContainer, contentColor = cs.onErrorContainer),
                ) { Icon(Icons.Rounded.DeleteOutline, stringResource(R.string.paths_delete)) }
            }
        }
    }

    if (showDelete && path != null) {
        ConfirmDeleteDialog(
            itemName = path.name,
            onConfirm = { showDelete = false; viewModel.delete(path.id); onBack() },
            onDismiss = { showDelete = false },
        )
    }
}

/** Elevation profile card: ascent/descent summary + a filled distance-vs-height chart. */
@Composable
private fun ElevationCard(path: SavedPath, metric: Boolean) {
    val cs = MaterialTheme.colorScheme
    TurboCard(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SectionLabel(stringResource(R.string.paths_elevation))
            Spacer(Modifier.weight(1f))
            Icon(Icons.Rounded.Terrain, null, tint = cs.primary, modifier = Modifier.size(16.dp))
            Text(" ${path.ascentText(metric)}", style = MaterialTheme.typography.labelLarge, color = cs.onSurface)
            Spacer(Modifier.size(10.dp))
            Icon(Icons.AutoMirrored.Rounded.TrendingDown, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(16.dp))
            Text(" ${path.descentText(metric)}", style = MaterialTheme.typography.labelLarge, color = cs.onSurfaceVariant)
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

/** Best-effort human filename for a content [uri], minus its extension. */
private fun displayName(context: Context, uri: android.net.Uri): String? {
    val name = context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
        val idx = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
        if (idx >= 0 && cursor.moveToFirst()) cursor.getString(idx) else null
    }
    return name?.substringBeforeLast('.')?.takeIf { it.isNotBlank() }
}

/** Write the track to a cache file in the chosen [format] and fire a share chooser. */
private fun shareTrack(context: Context, path: SavedPath, format: ExportFormat) {
    val dir = File(context.cacheDir, "tracks").apply { mkdirs() }
    val file = File(dir, exportFileName(path.name, format))
    file.writeText(serialize(path, format))
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    val send = Intent(Intent.ACTION_SEND).apply {
        type = format.mimeType
        putExtra(Intent.EXTRA_STREAM, uri)
        // ClipData grants the read permission to the chooser preview + target alike.
        clipData = ClipData.newRawUri(path.name, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(send, context.getString(R.string.paths_share_format, format.label)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
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
