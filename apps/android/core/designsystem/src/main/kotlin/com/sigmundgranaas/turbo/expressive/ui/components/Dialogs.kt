package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.designsystem.R

/**
 * Expressive confirmation dialog. A hero icon, title + body, and a confirm/dismiss
 * pair. [destructive] swaps the confirm button to the error palette.
 */
@Composable
fun TurboConfirmDialog(
    title: String,
    body: String,
    confirmLabel: String,
    icon: ImageVector,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    dismissLabel: String = stringResource(R.string.ds_cancel),
    destructive: Boolean = false,
) {
    val cs = MaterialTheme.colorScheme
    AlertDialog(
        onDismissRequest = onDismiss,
        modifier = modifier,
        icon = { Icon(icon, null, tint = if (destructive) cs.error else cs.primary, modifier = Modifier.size(28.dp)) },
        title = { Text(title, style = MaterialTheme.typography.headlineSmall) },
        text = { Text(body, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant) },
        confirmButton = {
            Button(
                onClick = onConfirm,
                colors = if (destructive) {
                    ButtonDefaults.buttonColors(containerColor = cs.error, contentColor = cs.onError)
                } else {
                    ButtonDefaults.buttonColors()
                },
            ) { Text(confirmLabel) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(dismissLabel) } },
    )
}

/** Confirm dialog for deleting a named marker. */
@Composable
fun DeleteMarkerDialog(markerName: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    TurboConfirmDialog(
        title = stringResource(R.string.ds_delete_marker_title),
        body = stringResource(R.string.ds_delete_marker_body, markerName),
        confirmLabel = stringResource(R.string.ds_delete),
        icon = Icons.Rounded.DeleteOutline,
        destructive = true,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
    )
}

/**
 * Generic "delete [named thing]?" confirmation, so every destructive delete
 * across the app (collections, tracks, offline regions, …) gets the same
 * guard + wording as [DeleteMarkerDialog].
 */
@Composable
fun ConfirmDeleteDialog(itemName: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    TurboConfirmDialog(
        title = stringResource(R.string.ds_delete_title),
        body = stringResource(R.string.ds_delete_body, itemName),
        confirmLabel = stringResource(R.string.ds_delete),
        icon = Icons.Rounded.DeleteOutline,
        destructive = true,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
    )
}

/**
 * Single-field "name this thing" dialog, shared by save-route and rename-track so
 * every name-entry flow looks and behaves the same. Confirm is disabled while the
 * field is blank; [onConfirm] receives the trimmed text.
 */
@Composable
fun NameInputDialog(
    title: String,
    confirmLabel: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
    initial: String = "",
) {
    var text by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, style = MaterialTheme.typography.headlineSmall) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text(stringResource(R.string.ds_name)) },
                singleLine = true,
            )
        },
        confirmButton = {
            Button(onClick = { onConfirm(text.trim()) }, enabled = text.isNotBlank()) { Text(confirmLabel) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.ds_cancel)) } },
    )
}

/**
 * Rationale dialog shown before requesting the location permission. [onAllow]
 * should trigger the actual system permission request; [onDismiss] dismisses.
 */
@Composable
fun LocationPermissionDialog(onAllow: () -> Unit, onDismiss: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = {
            Row(horizontalArrangement = Arrangement.Center) {
                Cookie(size = 56.dp, fill = cs.primaryContainer) {
                    Icon(Icons.Rounded.LocationOn, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(28.dp))
                }
            }
        },
        title = { Text(stringResource(R.string.ds_loc_title), style = MaterialTheme.typography.headlineSmall) },
        text = {
            Text(
                stringResource(R.string.ds_loc_body),
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        },
        confirmButton = { Button(onClick = onAllow) { Text(stringResource(R.string.ds_loc_allow)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.ds_loc_not_now)) } },
    )
}
