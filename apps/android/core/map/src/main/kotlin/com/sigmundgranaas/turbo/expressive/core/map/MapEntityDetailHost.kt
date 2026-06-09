package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.pressScale
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
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(start = 24.dp, end = 24.dp, bottom = 32.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (selection.icon != null) {
                    Cookie(size = 64.dp, fill = cs.primaryContainer) {
                        Icon(selection.icon, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(30.dp))
                    }
                    Spacer(Modifier.size(16.dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(selection.title, style = MaterialTheme.typography.headlineSmall, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (selection.subtitle != null) {
                        Text(selection.subtitle, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                    }
                }
            }

            selection.body?.let { body ->
                Spacer(Modifier.height(16.dp))
                body()
            }

            if (actions.isNotEmpty()) {
                Spacer(Modifier.height(20.dp))
                // Primary action gets a full-width row so its label never wraps/clips;
                // the rest spread evenly on a second row (fits any phone width).
                val primary = actions.first()
                val primarySource = remember { MutableInteractionSource() }
                Button(
                    onClick = { primary.onInvoke(ctx); state.clear() },
                    interactionSource = primarySource,
                    modifier = Modifier.fillMaxWidth().height(52.dp).pressScale(primarySource),
                ) {
                    Icon(primary.icon, null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.size(8.dp))
                    Text(primary.label, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                // Secondary actions are LABELLED (icon + text), not bare icons, so
                // Edit / Add-to-collection / Delete read at a glance; each springs on press.
                val secondary = actions.drop(1)
                if (secondary.isNotEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        secondary.forEach { action ->
                            val source = remember(action.label) { MutableInteractionSource() }
                            FilledTonalButton(
                                onClick = { action.onInvoke(ctx); state.clear() },
                                interactionSource = source,
                                colors = if (action.isDestructive) {
                                    ButtonDefaults.filledTonalButtonColors(containerColor = cs.errorContainer, contentColor = cs.onErrorContainer)
                                } else {
                                    ButtonDefaults.filledTonalButtonColors(containerColor = cs.secondaryContainer, contentColor = cs.onSecondaryContainer)
                                },
                                contentPadding = PaddingValues(horizontal = 12.dp),
                                modifier = Modifier.weight(1f).height(52.dp).pressScale(source),
                            ) {
                                Icon(action.icon, null, modifier = Modifier.size(18.dp))
                                Spacer(Modifier.size(6.dp))
                                Text(action.label, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}
