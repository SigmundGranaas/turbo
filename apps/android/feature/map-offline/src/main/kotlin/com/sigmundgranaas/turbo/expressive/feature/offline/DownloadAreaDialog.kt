package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.DetailLevel
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate

/**
 * Pre-flight confirm for "Download this area": a Standard/Detailed zoom-depth
 * choice with a live size + tile-count estimate, so the user isn't committing
 * blind, and a **disabled** download when the area is too large (the
 * [OfflineEstimate.withinLimits] guard), nudging them to zoom in. This is the
 * gate between the layers sheet's "Download this area" and the actual download.
 */
@Composable
fun DownloadAreaDialog(
    estimateFor: (DetailLevel) -> OfflineEstimate,
    onConfirm: (DetailLevel) -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var detail by remember { mutableStateOf(DetailLevel.Standard) }
    val estimate = remember(detail) { estimateFor(detail) }
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
            Column {
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    DetailLevel.entries.forEachIndexed { index, level ->
                        SegmentedButton(
                            selected = detail == level,
                            onClick = { detail = level },
                            shape = SegmentedButtonDefaults.itemShape(index = index, count = DetailLevel.entries.size),
                            modifier = Modifier.testTag("detail_${level.name}"),
                        ) {
                            Text(
                                stringResource(
                                    when (level) {
                                        DetailLevel.Standard -> R.string.offline_detail_standard
                                        DetailLevel.Detailed -> R.string.offline_detail_detailed
                                    },
                                ),
                            )
                        }
                    }
                }
                Spacer(Modifier.height(12.dp))
                Text(
                    if (ok) {
                        stringResource(R.string.offline_download_estimate, formatSize(estimate.bytes), estimate.tiles)
                    } else {
                        stringResource(R.string.offline_download_too_large, formatSize(estimate.bytes))
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = cs.onSurfaceVariant,
                )
                // Said once, plainly, and only when it is true. The size
                // above already includes the pack; this is the sentence
                // that explains why the number grew and what it buys —
                // the alternative being a user who notices the megabytes
                // and not the capability.
                //
                // And when it is NOT true, said just as plainly. A
                // region can be small enough to download and too big for
                // one routing pack — the two caps bound different things
                // — and the difference is invisible until the user is
                // standing in it without signal. An absent line would
                // read as an oversight; this one is the answer to "why
                // did routing work in the other area I saved".
                if (ok && estimate.packBytes > 0L) {
                    Spacer(Modifier.height(6.dp))
                    Text(
                        stringResource(R.string.offline_download_includes_routing),
                        style = MaterialTheme.typography.bodySmall,
                        color = cs.onSurfaceVariant,
                        modifier = Modifier.testTag("routingIncluded"),
                    )
                } else if (ok && estimate.routingOmittedForSize) {
                    Spacer(Modifier.height(6.dp))
                    Text(
                        stringResource(R.string.offline_download_no_routing),
                        style = MaterialTheme.typography.bodySmall,
                        color = cs.error,
                        modifier = Modifier.testTag("routingExcluded"),
                    )
                }
            }
        },
        confirmButton = {
            Button(onClick = { onConfirm(detail) }, enabled = ok, modifier = Modifier.testTag("downloadConfirm")) {
                Text(stringResource(R.string.offline_download_confirm))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.offline_download_cancel)) } },
    )
}
