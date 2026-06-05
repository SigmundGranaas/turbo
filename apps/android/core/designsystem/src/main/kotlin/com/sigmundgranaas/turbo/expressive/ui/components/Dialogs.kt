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
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp

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
    dismissLabel: String = "Cancel",
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
        title = "Delete marker?",
        body = "“$markerName” will be removed from your map. This can't be undone.",
        confirmLabel = "Delete",
        icon = Icons.Rounded.DeleteOutline,
        destructive = true,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
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
        title = { Text("Show your location", style = MaterialTheme.typography.headlineSmall) },
        text = {
            Text(
                "Turbo uses your location to center the map, follow you while navigating, and record tracks. " +
                    "Your position never leaves the device unless you share a track.",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        },
        confirmButton = { Button(onClick = onAllow) { Text("Allow location") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Not now") } },
    )
}
