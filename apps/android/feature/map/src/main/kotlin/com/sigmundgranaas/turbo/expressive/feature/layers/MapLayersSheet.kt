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
import androidx.compose.material.icons.rounded.Air
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Satellite
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Waves
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
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
    // Procedural weather-clouds overlay (wgpu engine only). The row is hidden
    // unless [cloudsAvailable]; toggling drives the bottom scrubber.
    cloudsAvailable: Boolean = false,
    cloudsOn: Boolean = false,
    onToggleClouds: (Boolean) -> Unit = {},
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
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                LayerCard(BaseLayer.Norgeskart, Icons.Rounded.Terrain, selected == BaseLayer.Norgeskart, Modifier.weight(1f)) { onSelectBase(BaseLayer.Norgeskart) }
                LayerCard(BaseLayer.Osm, Icons.Rounded.Map, selected == BaseLayer.Osm, Modifier.weight(1f)) { onSelectBase(BaseLayer.Osm) }
                LayerCard(BaseLayer.Satellite, Icons.Rounded.Satellite, selected == BaseLayer.Satellite, Modifier.weight(1f)) { onSelectBase(BaseLayer.Satellite) }
            }

            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.layers_overlays))
            Spacer(Modifier.height(8.dp))
            MapStyles.renderableOverlays.forEach { overlay ->
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
