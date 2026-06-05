package com.sigmundgranaas.turbo.expressive.feature.markers

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon

private val SheetShape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewMarkerSheet(
    position: LatLng,
    onDismiss: () -> Unit,
    onSave: (name: String, kind: ActivityKindId) -> Unit = { _, _ -> },
) {
    val cs = MaterialTheme.colorScheme
    var selectedKind by remember { mutableStateOf(ActivityKindId.Cabin) }
    val name = "Sjurfjellet"

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = SheetShape,
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(horizontal = 24.dp).padding(bottom = 24.dp)) {
            Text("New Marker", style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Text(formatCoords(position), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)

            Spacer(Modifier.height(18.dp))
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = TurboRadius.m, topEnd = TurboRadius.m))
                    .background(cs.surfaceContainerHigh)
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            ) {
                Text("Name", style = MaterialTheme.typography.labelMedium, color = cs.primary)
                Text(name, style = MaterialTheme.typography.bodyLarge, color = cs.onSurface)
            }
            Box(Modifier.fillMaxWidth().height(2.dp).background(cs.primary))

            Spacer(Modifier.height(22.dp))
            SectionLabel("Icon")
            Spacer(Modifier.height(12.dp))
            LazyVerticalGrid(
                columns = GridCells.Fixed(6),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.height(120.dp),
                userScrollEnabled = false,
            ) {
                items(ActivityKindId.entries) { kind ->
                    val sel = kind == selectedKind
                    Box(
                        Modifier
                            .size(48.dp)
                            .clip(RoundedCornerShape(if (sel) TurboRadius.l else TurboRadius.m))
                            .background(if (sel) cs.primary else cs.surfaceContainerHigh)
                            .clickable { selectedKind = kind },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(kind.icon, kind.label, tint = if (sel) cs.onPrimary else cs.primary, modifier = Modifier.size(22.dp))
                    }
                }
            }

            Spacer(Modifier.height(22.dp))
            Button(
                onClick = { onSave(name, selectedKind) },
                modifier = Modifier.fillMaxWidth().height(56.dp),
            ) { Text("Save Marker", style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700)) }
        }
    }
}
