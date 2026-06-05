package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription

/** One entry in the map speed-dial. */
data class FabAction(val label: String, val icon: ImageVector, val onClick: () -> Unit)

/**
 * The Expressive FAB speed-dial used on the map: a [ToggleFloatingActionButton]
 * that morphs +→× and expands a column of labelled mini-FABs. Uses the
 * first-party M3 Expressive [FloatingActionButtonMenu].
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MapFabMenu(
    expanded: Boolean,
    onExpandedChange: (Boolean) -> Unit,
    actions: List<FabAction>,
    modifier: Modifier = Modifier,
) {
    FloatingActionButtonMenu(
        modifier = modifier,
        expanded = expanded,
        button = {
            ToggleFloatingActionButton(
                checked = expanded,
                onCheckedChange = onExpandedChange,
                modifier = Modifier.semantics { stateDescription = if (expanded) "Expanded" else "Collapsed" },
            ) {
                Icon(
                    imageVector = if (expanded) Icons.Rounded.Close else Icons.Rounded.Add,
                    contentDescription = if (expanded) "Close menu" else "Open actions",
                )
            }
        },
    ) {
        actions.forEach { action ->
            FloatingActionButtonMenuItem(
                onClick = {
                    onExpandedChange(false)
                    action.onClick()
                },
                text = { Text(action.label) },
                icon = { Icon(action.icon, contentDescription = null) },
            )
        }
    }
}
