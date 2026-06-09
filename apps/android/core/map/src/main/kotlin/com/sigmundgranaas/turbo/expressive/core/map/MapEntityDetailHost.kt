package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.pressScaleClickable
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
                // A compact row of icon-over-label chips, like a map place card: every
                // action stays labelled yet uniform, the primary verb is emphasised, and
                // the row wraps gracefully when an entity exposes many actions — so we
                // never cram N equal-width text buttons that can't fit the screen.
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    actions.forEachIndexed { index, action ->
                        ActionChip(
                            label = action.label,
                            icon = action.icon,
                            emphasized = index == 0,
                            destructive = action.isDestructive,
                            onClick = { action.onInvoke(ctx); state.clear() },
                        )
                    }
                }
            }
        }
    }
}

/**
 * One action rendered as an icon-in-a-circle over its label — uniform width so any
 * number of them line up and wrap cleanly. The primary verb is [emphasized] (filled
 * primary), destructive actions go error-tinted; the whole circle springs on press.
 */
@Composable
private fun ActionChip(
    label: String,
    icon: ImageVector,
    emphasized: Boolean,
    destructive: Boolean,
    onClick: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val (container, content) = when {
        destructive -> cs.errorContainer to cs.onErrorContainer
        emphasized -> cs.primary to cs.onPrimary
        else -> cs.secondaryContainer to cs.onSecondaryContainer
    }
    Column(
        modifier = Modifier.width(70.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(56.dp)
                .clip(CircleShape)
                .background(container)
                .pressScaleClickable(onClick = onClick, onClickLabel = label, role = Role.Button),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, null, tint = content, modifier = Modifier.size(24.dp))
        }
        Spacer(Modifier.height(6.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurface,
            maxLines = 2,
            textAlign = TextAlign.Center,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
