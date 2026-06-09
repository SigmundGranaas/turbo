package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.DownloadDone
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PauseCircle
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.components.ConfirmDeleteDialog
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OfflineMapsScreen(
    onBack: () -> Unit,
    viewModel: OfflineViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val regions by viewModel.regions.collectAsStateWithLifecycle()
    val totalBytes = regions.sumOf { it.sizeBytes }
    var pendingDelete by remember { mutableStateOf<OfflineRegionInfo?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.offline_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.offline_back))
                    }
                },
            )
        },
    ) { padding ->
        if (regions.isEmpty()) {
            EmptyState(
                icon = Icons.Rounded.CloudOff,
                title = stringResource(R.string.offline_empty_title),
                body = stringResource(R.string.offline_empty_body),
                modifier = Modifier.fillMaxSize().padding(padding),
            )
        } else {
            LazyColumn(
                contentPadding = padding,
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp),
            ) {
                item {
                    Text(
                        pluralStringResource(R.plurals.offline_summary, regions.size, regions.size, formatSize(totalBytes)),
                        style = MaterialTheme.typography.titleSmall,
                        color = cs.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                    )
                }
                items(regions, key = { it.id }) { region ->
                    RegionCard(
                        region = region,
                        onDelete = { pendingDelete = region },
                        onRetry = { viewModel.retry(region.id) },
                        onPause = { viewModel.pause(region.id) },
                        onResume = { viewModel.resume(region.id) },
                    )
                }
                item { Spacer(Modifier.height(16.dp)) }
            }
        }
    }

    pendingDelete?.let { target ->
        ConfirmDeleteDialog(
            itemName = target.name,
            onConfirm = { viewModel.delete(target.id); pendingDelete = null },
            onDismiss = { pendingDelete = null },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun RegionCard(
    region: OfflineRegionInfo,
    onDelete: () -> Unit,
    onRetry: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val regionDesc = stringResource(R.string.offline_region_desc, region.name)
    Surface(
        shape = RoundedCornerShape(TurboRadius.l),
        color = cs.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth().semantics { contentDescription = regionDesc },
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    region.name,
                    style = MaterialTheme.typography.titleMedium,
                    color = cs.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                // The base map differentiates same-named regions (e.g. two "Tromsø").
                Text(
                    region.base.title,
                    style = MaterialTheme.typography.labelSmall,
                    color = cs.onSurfaceVariant,
                )
                Spacer(Modifier.height(6.dp))
                when (region.status) {
                    OfflineStatus.Complete -> StatusLine(
                        icon = Icons.Rounded.DownloadDone,
                        tint = cs.primary,
                        text = stringResource(R.string.offline_downloaded, formatSize(region.sizeBytes)),
                    )
                    OfflineStatus.Paused -> StatusLine(
                        icon = Icons.Rounded.PauseCircle,
                        tint = cs.onSurfaceVariant,
                        text = stringResource(R.string.offline_paused, (region.progress * 100).toInt()),
                    )
                    OfflineStatus.Failed -> {
                        StatusLine(
                            icon = Icons.Rounded.ErrorOutline,
                            tint = cs.error,
                            text = region.errorReason?.let { stringResource(R.string.offline_failed_reason, it) }
                                ?: stringResource(R.string.offline_failed),
                        )
                        TextButton(onClick = onRetry, contentPadding = PaddingValues(horizontal = 4.dp)) {
                            Icon(Icons.Rounded.Refresh, null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.size(6.dp))
                            Text(stringResource(R.string.offline_retry))
                        }
                    }
                    OfflineStatus.Downloading -> {
                        LinearWavyProgressIndicator(
                            progress = { region.progress },
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Spacer(Modifier.height(6.dp))
                        Text(
                            stringResource(R.string.offline_downloading, (region.progress * 100).toInt()),
                            style = MaterialTheme.typography.bodySmall,
                            color = cs.onSurfaceVariant,
                        )
                    }
                }
            }
            Spacer(Modifier.size(8.dp))
            when (region.status) {
                OfflineStatus.Downloading -> IconButton(onClick = onPause) {
                    Icon(Icons.Rounded.Pause, stringResource(R.string.offline_pause, region.name), tint = cs.onSurfaceVariant)
                }
                OfflineStatus.Paused -> IconButton(onClick = onResume) {
                    Icon(Icons.Rounded.PlayArrow, stringResource(R.string.offline_resume, region.name), tint = cs.primary)
                }
                else -> Unit
            }
            IconButton(onClick = onDelete) {
                Icon(Icons.Rounded.Delete, stringResource(R.string.offline_delete, region.name), tint = cs.error)
            }
        }
    }
}

/** A small icon + caption row reused by the completed / paused / failed states. */
@Composable
private fun StatusLine(icon: androidx.compose.ui.graphics.vector.ImageVector, tint: androidx.compose.ui.graphics.Color, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, tint = tint, modifier = Modifier.size(16.dp))
        Spacer(Modifier.size(6.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

internal fun formatSize(bytes: Long): String = when {
    bytes >= 1_000_000_000 -> "%.1f GB".format(bytes / 1_000_000_000.0)
    bytes >= 1_000_000 -> "%.1f MB".format(bytes / 1_000_000.0)
    bytes >= 1_000 -> "%.0f kB".format(bytes / 1_000.0)
    else -> "$bytes B"
}
