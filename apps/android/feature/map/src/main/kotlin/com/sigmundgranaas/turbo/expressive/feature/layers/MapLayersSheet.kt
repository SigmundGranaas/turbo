package com.sigmundgranaas.turbo.expressive.feature.layers

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AcUnit
import androidx.compose.material.icons.rounded.Air
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Satellite
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.material.icons.rounded.Waves
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MapLayersSheet(
    selected: BaseLayer,
    overlays: Set<OverlayId>,
    onSelectBase: (BaseLayer) -> Unit,
    onToggleOverlay: (OverlayId) -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, bottom = 32.dp)) {
            Text("Map Layers", style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Spacer(Modifier.height(18.dp))
            SectionLabel("Base map")
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                LayerCard(BaseLayer.Norgeskart, Icons.Rounded.Terrain, selected == BaseLayer.Norgeskart, Modifier.weight(1f)) { onSelectBase(BaseLayer.Norgeskart) }
                LayerCard(BaseLayer.Osm, Icons.Rounded.Map, selected == BaseLayer.Osm, Modifier.weight(1f)) { onSelectBase(BaseLayer.Osm) }
                LayerCard(BaseLayer.Satellite, Icons.Rounded.Satellite, selected == BaseLayer.Satellite, Modifier.weight(1f)) { onSelectBase(BaseLayer.Satellite) }
            }

            Spacer(Modifier.height(24.dp))
            SectionLabel("Overlays")
            Spacer(Modifier.height(8.dp))
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh).padding(horizontal = 18.dp)) {
                OverlayRow(OverlayId.Waves, Icons.Rounded.Waves, OverlayId.Waves in overlays) { onToggleOverlay(OverlayId.Waves) }
                HorizontalDivider(color = cs.outlineVariant)
                OverlayRow(OverlayId.Wind, Icons.Rounded.Air, OverlayId.Wind in overlays) { onToggleOverlay(OverlayId.Wind) }
                HorizontalDivider(color = cs.outlineVariant)
                OverlayRow(OverlayId.Avalanche, Icons.Rounded.AcUnit, OverlayId.Avalanche in overlays) { onToggleOverlay(OverlayId.Avalanche) }
            }
        }
    }
}

@Composable
private fun OverlayRow(id: OverlayId, icon: ImageVector, on: Boolean, onToggle: () -> Unit) {
    ListRowItem(icon = icon, title = id.title, subtitle = id.subtitle, trailing = { Switch(checked = on, onCheckedChange = { onToggle() }) })
}

@Composable
private fun LayerCard(layer: BaseLayer, icon: ImageVector, selected: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier
            .height(116.dp)
            .clip(RoundedCornerShape(TurboRadius.xl))
            .background(cs.surfaceContainerHigh)
            .border(
                width = if (selected) 3.dp else 1.dp,
                color = if (selected) cs.primary else cs.outlineVariant,
                shape = RoundedCornerShape(TurboRadius.xl),
            )
            .clickable(onClick = onClick),
    ) {
        Column(Modifier.padding(10.dp).align(Alignment.BottomStart), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Icon(icon, null, tint = if (selected) cs.primary else cs.onSurfaceVariant, modifier = Modifier.size(20.dp))
            Text(layer.title, style = MaterialTheme.typography.labelLarge, color = if (selected) cs.primary else cs.onSurface)
        }
        if (selected) {
            Box(
                Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).clip(CircleShape).background(cs.primary),
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.Rounded.Check, null, tint = cs.onPrimary, modifier = Modifier.size(16.dp)) }
        }
    }
}
