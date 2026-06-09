package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.foundation.interaction.MutableInteractionSource
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
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
import androidx.compose.ui.res.stringResource
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
                ActionBar(actions = actions, onInvoke = { it.onInvoke(ctx); state.clear() })
            }
        }
    }
}

/**
 * The detail action bar: ONE big expressive primary button (icon + label), ONE
 * square icon-only quick action, and the rest tucked into an overflow menu (a
 * list, not strewn across the sheet). Degrades cleanly when there are fewer
 * actions — primary only, or primary + quick with no overflow.
 */
@Composable
private fun ActionBar(actions: List<MapEntityAction>, onInvoke: (MapEntityAction) -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val primary = actions.first()
        val primarySource = remember { MutableInteractionSource() }
        Button(
            onClick = { onInvoke(primary) },
            interactionSource = primarySource,
            modifier = Modifier.weight(1f).height(56.dp).pressScale(primarySource),
        ) {
            Icon(primary.icon, null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.size(8.dp))
            Text(primary.label, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }

        val rest = actions.drop(1)
        // One square quick action (the next-most-useful verb).
        rest.firstOrNull()?.let { quick ->
            val quickSource = remember { MutableInteractionSource() }
            FilledTonalIconButton(
                onClick = { onInvoke(quick) },
                interactionSource = quickSource,
                shape = RoundedCornerShape(18.dp),
                modifier = Modifier.size(56.dp).pressScale(quickSource),
            ) { Icon(quick.icon, quick.label, modifier = Modifier.size(22.dp)) }
        }

        // Everything else lives behind an overflow "⋮" → a dropdown list.
        val overflow = rest.drop(1)
        if (overflow.isNotEmpty()) {
            val moreSource = remember { MutableInteractionSource() }
            var menuOpen by remember { mutableStateOf(false) }
            Box {
                FilledTonalIconButton(
                    onClick = { menuOpen = true },
                    interactionSource = moreSource,
                    shape = RoundedCornerShape(18.dp),
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = cs.surfaceContainerHighest,
                        contentColor = cs.onSurface,
                    ),
                    modifier = Modifier.size(56.dp).pressScale(moreSource),
                ) { Icon(Icons.Rounded.MoreVert, stringResource(R.string.me_more_actions), modifier = Modifier.size(22.dp)) }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    overflow.forEach { action ->
                        val tint = if (action.isDestructive) cs.error else cs.onSurface
                        DropdownMenuItem(
                            text = { Text(action.label, color = tint) },
                            leadingIcon = { Icon(action.icon, null, tint = tint) },
                            onClick = { menuOpen = false; onInvoke(action) },
                        )
                    }
                }
            }
        }
    }
}
