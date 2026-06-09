package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.feature.map.R

/**
 * Pre-flight confirm for "Download this area": shows the estimated size + tile count
 * so the user isn't committing blind, and **disables** the download when the area is
 * too large (the [OfflineEstimate.withinLimits] guard), nudging them to zoom in. This
 * is the gate between the layers sheet's "Download this area" and the actual download.
 */
@Composable
internal fun DownloadAreaDialog(
    estimate: OfflineEstimate,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val ok = estimate.withinLimits
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = {
            Icon(
                if (ok) Icons.Rounded.CloudDownload else Icons.Rounded.WarningAmber,
                null,
                tint = if (ok) cs.primary else cs.error,
                modifier = Modifier.size(28.dp),
            )
        },
        title = { Text(stringResource(R.string.offline_download_title), style = MaterialTheme.typography.headlineSmall) },
        text = {
            Text(
                if (ok) {
                    stringResource(R.string.offline_download_estimate, formatSize(estimate.bytes), estimate.tiles)
                } else {
                    stringResource(R.string.offline_download_too_large, formatSize(estimate.bytes))
                },
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        },
        confirmButton = {
            Button(onClick = onConfirm, enabled = ok, modifier = Modifier.testTag("downloadConfirm")) {
                Text(stringResource(R.string.offline_download_confirm))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.offline_download_cancel)) } },
    )
}
