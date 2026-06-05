package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * The one detail host: renders the current [MapSelectionState.selection] as an
 * Expressive bottom sheet — title/subtitle, the selection's optional [body], and
 * an action bar assembled from the [MapEntityActionRegistry] (standard actions +
 * the entity's `extraActions`). Replaces per-feature info sheets, so the map
 * shell no longer depends on individual feature modules.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MapEntityDetailHost(
    state: MapSelectionState,
    registry: MapEntityActionRegistry,
) {
    val selection = state.selection ?: return
    val cs = MaterialTheme.colorScheme
    val ctx = selection.toActionContext()
    val actions = registry.availableFor(ctx, selection.extraActions, selection.includeStandardActions)

    ModalBottomSheet(
        onDismissRequest = state::clear,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, bottom = 32.dp)) {
            Text(selection.title, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (selection.subtitle != null) {
                Text(selection.subtitle, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }

            selection.body?.let { body ->
                Spacer(Modifier.height(16.dp))
                body()
            }

            if (actions.isNotEmpty()) {
                Spacer(Modifier.height(20.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    val primary = actions.first()
                    Button(
                        onClick = { primary.onInvoke(ctx); state.clear() },
                        modifier = Modifier.weight(1f).height(52.dp),
                    ) {
                        Icon(primary.icon, null, modifier = Modifier.size(20.dp))
                        Spacer(Modifier.size(8.dp))
                        Text(primary.label, style = MaterialTheme.typography.titleMedium)
                    }
                    actions.drop(1).forEach { action ->
                        FilledIconButton(
                            onClick = { action.onInvoke(ctx); state.clear() },
                            modifier = Modifier.size(52.dp),
                            colors = if (action.isDestructive) {
                                IconButtonDefaults.filledIconButtonColors(containerColor = cs.errorContainer, contentColor = cs.onErrorContainer)
                            } else {
                                IconButtonDefaults.filledIconButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer)
                            },
                        ) { Icon(action.icon, action.label) }
                    }
                }
            }
        }
    }
}
