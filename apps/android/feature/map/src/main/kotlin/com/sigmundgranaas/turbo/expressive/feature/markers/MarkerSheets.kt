package com.sigmundgranaas.turbo.expressive.feature.markers

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import com.sigmundgranaas.turbo.expressive.feature.map.R
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes

private val SheetShape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl)

/** Preset pin tints offered in the editor. `null` = the kind's terracotta default. */
private val MarkerColors: List<Long?> = listOf(
    null,
    0xFFE0432B, // red
    0xFFEF6C00, // orange
    0xFF2E7D32, // green
    0xFF1A73E8, // blue
    0xFF7B1FA2, // purple
    0xFF00838F, // teal
)

/**
 * Create or edit a map marker. Pass [existing] to edit (prefills name/kind/colour/notes
 * and keeps its id via the caller); leave null to create at [position]. The same sheet
 * backs both flows so the create/edit UX never drifts.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MarkerEditorSheet(
    position: LatLng,
    onDismiss: () -> Unit,
    existing: Marker? = null,
    suggestedName: String? = null,
    suggestedSubtitle: String? = null,
    onSave: (name: String, kind: ActivityKindId, colorArgb: Long?, notes: String?) -> Unit = { _, _, _, _ -> },
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = SheetShape,
        containerColor = cs.surfaceContainerLow,
    ) {
        // No conditions block here — the long-press menu already shows the spot's
        // weather, so a forecast card on the new-marker editor was redundant chrome.
        MarkerEditorContent(
            position = position,
            existing = existing,
            suggestedName = suggestedName,
            suggestedSubtitle = suggestedSubtitle,
            onSave = onSave,
        )
    }
}

/** The editor body, hoisted out of the sheet so it can be exercised headlessly. */
@Composable
internal fun MarkerEditorContent(
    position: LatLng,
    existing: Marker?,
    suggestedName: String? = null,
    suggestedSubtitle: String? = null,
    conditions: @Composable () -> Unit = {},
    onSave: (name: String, kind: ActivityKindId, colorArgb: Long?, notes: String?) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var name by rememberSaveable { mutableStateOf(existing?.name.orEmpty()) }
    var nameEdited by rememberSaveable { mutableStateOf(false) }
    var notes by rememberSaveable { mutableStateOf(existing?.notes.orEmpty()) }
    var selectedKind by remember { mutableStateOf(existing?.kind ?: ActivityKindId.Cabin) }
    var color by remember { mutableStateOf(existing?.colorArgb) }
    val accent = color?.let { Color(it) } ?: cs.primary

    // A reverse-geocoded name resolves asynchronously after the sheet opens; adopt
    // it only for a brand-new marker the user hasn't started naming themselves.
    LaunchedEffect(suggestedName) {
        if (existing == null && !nameEdited && !suggestedName.isNullOrBlank() && name.isEmpty()) {
            name = suggestedName
        }
    }

    Column(
        Modifier
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 24.dp)
            .padding(bottom = 24.dp),
    ) {
            Text(stringResource(if (existing == null) R.string.marker_new else R.string.marker_edit), style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            Text(formatCoords(position), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            if (existing == null && !suggestedSubtitle.isNullOrBlank()) {
                Text(suggestedSubtitle, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }

            // Conditions at this point — long-pressing the map answers "what's the
            // weather here?" without first creating + tapping a marker. Passed as a
            // slot so this body stays Hilt-free + headlessly testable.
            conditions()

            Spacer(Modifier.height(18.dp))
            OutlinedTextField(
                value = name,
                onValueChange = { name = it; nameEdited = true },
                label = { Text(stringResource(R.string.marker_name)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().testTag("markerName"),
            )

            Spacer(Modifier.height(12.dp))
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text(stringResource(R.string.marker_notes)) },
                minLines = 2,
                maxLines = 4,
                modifier = Modifier.fillMaxWidth().testTag("markerNotes"),
            )

            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.marker_icon))
            Spacer(Modifier.height(12.dp))
            // FlowRow wraps all kinds and grows to fit — the sheet itself scrolls,
            // so no nested-scroll clipping (a fixed-height grid hid most icons).
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                maxItemsInEachRow = 6,
                modifier = Modifier.fillMaxWidth(),
            ) {
                ActivityKindId.entries.forEach { kind ->
                    val sel = kind == selectedKind
                    Box(
                        Modifier
                            .size(48.dp)
                            .clip(RoundedCornerShape(if (sel) TurboRadius.l else TurboRadius.m))
                            .background(if (sel) accent else cs.surfaceContainerHigh)
                            .clickable { selectedKind = kind },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(kind.icon, stringResource(kind.labelRes), tint = if (sel) cs.onPrimary else accent, modifier = Modifier.size(22.dp))
                    }
                }
            }

            Spacer(Modifier.height(22.dp))
            SectionLabel(stringResource(R.string.marker_colour))
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                MarkerColors.forEach { swatch ->
                    val swatchColor = swatch?.let { Color(it) } ?: cs.primary
                    val sel = swatch == color
                    Box(
                        Modifier
                            .size(34.dp)
                            .clip(CircleShape)
                            .background(swatchColor)
                            .border(
                                width = if (sel) 3.dp else 0.dp,
                                color = cs.onSurface,
                                shape = CircleShape,
                            )
                            .clickable { color = swatch },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (swatch == null) {
                            Text("A", style = MaterialTheme.typography.labelMedium, color = cs.onPrimary)
                        }
                        if (sel) Icon(Icons.Rounded.Check, null, tint = cs.onPrimary, modifier = Modifier.size(18.dp))
                    }
                }
            }

            Spacer(Modifier.height(24.dp))
            Button(
                onClick = { onSave(name.trim(), selectedKind, color, notes.trim().ifBlank { null }) },
                enabled = name.isNotBlank(),
                modifier = Modifier.fillMaxWidth().height(56.dp).testTag("markerSave"),
            ) {
                Text(
                    stringResource(if (existing == null) R.string.marker_save else R.string.marker_update),
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700),
                )
            }
        }
}
