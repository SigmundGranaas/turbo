package com.sigmundgranaas.turbo.expressive.feature.layers

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Air
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Satellite
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Waves
import androidx.compose.material.icons.rounded.WbSunny
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.CustomTileSource
import com.sigmundgranaas.turbo.expressive.domain.DEFAULT_3D_DETENT
import com.sigmundgranaas.turbo.expressive.domain.MAX_3D_EXAGGERATION
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.rememberTurboHaptics
import com.sigmundgranaas.turbo.expressive.ui.map.MapStyles
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MapLayersSheet(
    selected: BaseLayer,
    onSelectBase: (BaseLayer) -> Unit,
    onDownloadArea: () -> Unit,
    onDismiss: () -> Unit,
    activeOverlays: Set<OverlayId> = emptySet(),
    onToggleOverlay: (OverlayId, Boolean) -> Unit = { _, _ -> },
    // User-added XYZ basemaps ("add your own map URL"). When [selectedCustomId]
    // is set that source is the active base and the built-in cards deselect.
    customSources: List<CustomTileSource> = emptyList(),
    selectedCustomId: String? = null,
    onSelectCustom: (String?) -> Unit = {},
    onAddCustom: (name: String, urlTemplate: String) -> Unit = { _, _ -> },
    onRemoveCustom: (String) -> Unit = {},
    // Procedural weather-clouds overlay (wgpu engine only). The row is hidden
    // unless [cloudsAvailable]; toggling drives the bottom scrubber.
    cloudsAvailable: Boolean = false,
    cloudsOn: Boolean = false,
    onToggleClouds: (Boolean) -> Unit = {},
    // Hiking-trails overlay row is gated behind Settings → Experimental; hidden off.
    trailsAvailable: Boolean = true,
    // 3D-terrain slider: 0 = flat 2D (tilt locked); > 0 = 3D with this exaggeration.
    threeDLevel: Float = 0f,
    onThreeDLevel: (Float) -> Unit = {},
    // Sun slider: 0 = off; > 0 lights the DEM relief (works in 2D top-down and 3D).
    sunLevel: Float = 0f,
    onSunLevel: (Float) -> Unit = {},
) {
    val cs = MaterialTheme.colorScheme
    val haptics = rememberTurboHaptics()
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(start = 24.dp, end = 24.dp, bottom = 32.dp),
        ) {
            Text(stringResource(R.string.layers_title), style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Spacer(Modifier.height(18.dp))
            SectionLabel(stringResource(R.string.layers_base))
            Spacer(Modifier.height(12.dp))
            // A custom source being active deselects the built-in cards.
            val builtinSelected: (BaseLayer) -> Boolean = { selectedCustomId == null && selected == it }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                LayerCard(BaseLayer.Norgeskart, Icons.Rounded.Terrain, builtinSelected(BaseLayer.Norgeskart), Modifier.weight(1f)) { onSelectBase(BaseLayer.Norgeskart) }
                LayerCard(BaseLayer.Osm, Icons.Rounded.Map, builtinSelected(BaseLayer.Osm), Modifier.weight(1f)) { onSelectBase(BaseLayer.Osm) }
                LayerCard(BaseLayer.Satellite, Icons.Rounded.Satellite, builtinSelected(BaseLayer.Satellite), Modifier.weight(1f)) { onSelectBase(BaseLayer.Satellite) }
            }

            var showAddCustom by remember { mutableStateOf(false) }
            Spacer(Modifier.height(14.dp))
            customSources.forEach { source ->
                val active = source.id == selectedCustomId
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(TurboRadius.m))
                        .clickable { haptics.toggle(true); onSelectCustom(if (active) null else source.id) }
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                ) {
                    Icon(Icons.Rounded.Public, null, tint = if (active) cs.primary else cs.onSurfaceVariant, modifier = Modifier.size(22.dp))
                    Spacer(Modifier.size(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(source.name, style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                        Text(
                            source.urlTemplate,
                            style = MaterialTheme.typography.bodySmall,
                            color = cs.onSurfaceVariant,
                            maxLines = 1,
                            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                        )
                    }
                    if (active) {
                        Icon(Icons.Rounded.Check, null, tint = cs.primary, modifier = Modifier.size(20.dp))
                        Spacer(Modifier.size(8.dp))
                    }
                    Icon(
                        Icons.Rounded.Close,
                        stringResource(R.string.layers_custom_remove),
                        tint = cs.onSurfaceVariant,
                        modifier = Modifier.size(20.dp).clickable { onRemoveCustom(source.id) },
                    )
                }
            }
            androidx.compose.material3.TextButton(onClick = { showAddCustom = true }) {
                Icon(Icons.Rounded.Add, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.size(6.dp))
                Text(stringResource(R.string.layers_custom_add))
            }
            if (showAddCustom) {
                AddCustomMapDialog(
                    onConfirm = { name, url -> onAddCustom(name, url); showAddCustom = false },
                    onDismiss = { showAddCustom = false },
                )
            }

            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.layers_overlays))
            Spacer(Modifier.height(8.dp))
            // Hiking trails is experimental — drop its row unless the gate is on.
            val overlays = MapStyles.renderableOverlays.filter {
                it != OverlayId.Trails || trailsAvailable
            }
            overlays.forEach { overlay ->
                val (icon, titleRes, subRes) = overlayRow(overlay)
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Icon(icon, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                    Spacer(Modifier.size(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(titleRes), style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                        Text(stringResource(subRes), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
                    }
                    Switch(
                        checked = overlay in activeOverlays,
                        onCheckedChange = { on -> haptics.toggle(on); onToggleOverlay(overlay, on) },
                    )
                }
            }

            // Procedural weather clouds (wgpu engine only) — the enable lives here
            // now; the play/scrub control appears at the bottom of the map.
            if (cloudsAvailable) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Icon(Icons.Rounded.Cloud, null, tint = cs.primary, modifier = Modifier.size(22.dp))
                    Spacer(Modifier.size(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.layers_clouds), style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                        Text(stringResource(R.string.layers_clouds_sub), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
                    }
                    Switch(
                        checked = cloudsOn,
                        onCheckedChange = { on -> haptics.toggle(on); onToggleClouds(on) },
                    )
                }
            }

            // ── Terrain & light: the relocated 3D + Sun controls ──────────────
            // Both are pure sliders that never move the camera (the hard decoupling
            // rule): 3D unlocks tilt gestures + exaggerates relief; Sun lights the
            // relief top-down and works in 2D too.
            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.layers_terrain))
            Spacer(Modifier.height(4.dp))
            LayerSlider(
                icon = Icons.Rounded.Terrain,
                title = stringResource(R.string.layers_3d),
                subtitle = stringResource(R.string.layers_3d_sub),
                value = threeDLevel,
                valueRange = 0f..MAX_3D_EXAGGERATION,
                defaultOn = DEFAULT_3D_DETENT,
                onValueChange = onThreeDLevel,
            )
            LayerSlider(
                icon = Icons.Rounded.WbSunny,
                title = stringResource(R.string.layers_sun),
                subtitle = stringResource(R.string.layers_sun_sub),
                value = sunLevel,
                valueRange = 0f..1f,
                defaultOn = 0.5f,
                onValueChange = onSunLevel,
            )

            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.layers_offline))
            Spacer(Modifier.height(12.dp))
            FilledTonalButton(
                onClick = onDownloadArea,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(Icons.Rounded.Download, null, modifier = Modifier.size(20.dp))
                Spacer(Modifier.size(8.dp))
                Text(stringResource(R.string.layers_download_area))
            }
        }
    }
}

/** Name + XYZ URL form for a user-added basemap, with live template validation
 *  (http(s) + `{z}`/`{x}`/`{y}` — the same rule the web picker applies). */
@Composable
private fun AddCustomMapDialog(onConfirm: (name: String, url: String) -> Unit, onDismiss: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    var name by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    val urlValid = CustomTileSource.isValidTemplate(url)
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.layers_custom_add_title)) },
        text = {
            Column {
                androidx.compose.material3.OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.layers_custom_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(10.dp))
                androidx.compose.material3.OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.layers_custom_url)) },
                    placeholder = { Text("https://example.com/tiles/{z}/{x}/{y}.png") },
                    singleLine = true,
                    isError = url.isNotBlank() && !urlValid,
                    supportingText = {
                        if (url.isNotBlank() && !urlValid) {
                            Text(stringResource(R.string.layers_custom_url_invalid), color = cs.error)
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            androidx.compose.material3.TextButton(
                onClick = { onConfirm(name, url) },
                enabled = urlValid,
            ) { Text(stringResource(R.string.layers_custom_confirm)) }
        },
        dismissButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) {
                Text(stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_cancel))
            }
        },
    )
}

/**
 * A layer row with an enable switch and, when on, a fine-tune slider — used for
 * the 3D-terrain and Sun controls. The switch snaps [value] between 0 (off) and
 * [defaultOn] (a sensible detent), so enabling doesn't force a drag from zero; the
 * slider then tunes it. Neither control ever moves the camera — that decoupling
 * lives in the map's environment reducer, not here.
 */
@Composable
private fun LayerSlider(
    icon: ImageVector,
    title: String,
    subtitle: String,
    value: Float,
    valueRange: ClosedFloatingPointRange<Float>,
    defaultOn: Float,
    onValueChange: (Float) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val haptics = rememberTurboHaptics()
    val on = value > 0f
    Column(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Icon(icon, null, tint = if (on) cs.primary else cs.onSurfaceVariant, modifier = Modifier.size(22.dp))
            Spacer(Modifier.size(12.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
                Text(subtitle, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
            Switch(
                checked = on,
                onCheckedChange = { enabled -> haptics.toggle(enabled); onValueChange(if (enabled) defaultOn else 0f) },
            )
        }
        if (on) {
            Slider(
                value = value.coerceIn(valueRange.start, valueRange.endInclusive),
                onValueChange = onValueChange,
                valueRange = valueRange,
                modifier = Modifier.fillMaxWidth().padding(start = 34.dp),
            )
        }
    }
}

/** Icon + localized title/subtitle string-ids for an overlay row. */
private fun overlayRow(overlay: OverlayId): Triple<ImageVector, Int, Int> = when (overlay) {
    OverlayId.Trails -> Triple(Icons.Rounded.Hiking, R.string.layers_trails, R.string.layers_trails_sub)
    OverlayId.Avalanche -> Triple(Icons.Rounded.Terrain, R.string.layers_avalanche, R.string.layers_avalanche_sub)
    OverlayId.Waves -> Triple(Icons.Rounded.Waves, R.string.layers_waves, R.string.layers_waves_sub)
    OverlayId.Wind -> Triple(Icons.Rounded.Air, R.string.layers_wind, R.string.layers_wind_sub)
}

@Composable
private fun LayerCard(layer: BaseLayer, icon: ImageVector, selected: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier
            .height(116.dp)
            .clip(RoundedCornerShape(TurboRadius.xl))
            .border(
                width = if (selected) 3.dp else 1.dp,
                color = if (selected) cs.primary else cs.outlineVariant,
                shape = RoundedCornerShape(TurboRadius.xl),
            )
            .clickable(onClick = onClick),
    ) {
        // Map-like thumbnail crop — palette + contour strokes evoke each base map.
        LayerThumbnail(layer, Modifier.matchParentSize())
        // Bottom scrim so the label stays legible over the thumbnail.
        Box(
            Modifier.matchParentSize().background(
                Brush.verticalGradient(
                    0.45f to Color.Transparent,
                    1f to Color.Black.copy(alpha = 0.55f),
                ),
            ),
        )
        Column(Modifier.padding(10.dp).align(Alignment.BottomStart), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Icon(icon, null, tint = Color.White, modifier = Modifier.size(20.dp))
            Text(layer.title, style = MaterialTheme.typography.labelLarge, color = Color.White)
        }
        if (selected) {
            Box(
                Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).clip(CircleShape).background(cs.primary),
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.Rounded.Check, null, tint = cs.onPrimary, modifier = Modifier.size(16.dp)) }
        }
    }
}

/** A tiny faux-map crop per base layer: a palette gradient + a few contour/feature
 *  strokes, so each card previews the map's look without bundling raster tiles. */
@Composable
private fun LayerThumbnail(layer: BaseLayer, modifier: Modifier = Modifier) {
    val palette = when (layer) {
        // Topographic cream → tan with brown contour lines.
        BaseLayer.Norgeskart -> LayerPalette(Color(0xFFF3ECDD), Color(0xFFD9C7A0), Color(0xFF9C7B45), contours = true)
        // OSM neutral paper with grey roads + a green patch.
        BaseLayer.Osm -> LayerPalette(Color(0xFFEDEAE3), Color(0xFFDDE6CF), Color(0xFFB0B0B0), roads = true)
        // Satellite dark olive/forest mottling.
        BaseLayer.Satellite -> LayerPalette(Color(0xFF2E3B26), Color(0xFF44572F), Color(0xFF6E8048), satellite = true)
    }
    Canvas(modifier) {
        drawRect(Brush.linearGradient(listOf(palette.base, palette.mid)))
        val w = size.width
        val h = size.height
        if (palette.contours) {
            for (i in 1..4) {
                val y = h * (i / 5f)
                val path = Path().apply {
                    moveTo(0f, y)
                    cubicTo(w * 0.3f, y - h * 0.06f, w * 0.7f, y + h * 0.06f, w, y - h * 0.02f)
                }
                drawPath(path, palette.accent.copy(alpha = 0.55f), style = Stroke(width = 1.5f))
            }
        }
        if (palette.roads) {
            drawLine(palette.accent, Offset(0f, h * 0.7f), Offset(w, h * 0.35f), strokeWidth = 3f)
            drawLine(palette.accent, Offset(w * 0.4f, h), Offset(w * 0.6f, 0f), strokeWidth = 2f)
        }
        if (palette.satellite) {
            drawCircle(palette.accent.copy(alpha = 0.5f), radius = w * 0.25f, center = Offset(w * 0.3f, h * 0.35f))
            drawCircle(palette.accent.copy(alpha = 0.4f), radius = w * 0.18f, center = Offset(w * 0.75f, h * 0.7f))
        }
    }
}

private data class LayerPalette(
    val base: Color,
    val mid: Color,
    val accent: Color,
    val contours: Boolean = false,
    val roads: Boolean = false,
    val satellite: Boolean = false,
)
