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
import androidx.compose.foundation.clickable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.DeleteSweep
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.ui.components.NameInputDialog
import com.sigmundgranaas.turbo.expressive.ui.components.TurboConfirmDialog
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import kotlinx.coroutines.launch
import java.text.DateFormat
import java.util.Date

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OfflineMapsScreen(
    onBack: () -> Unit,
    viewModel: OfflineViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val regions by viewModel.regions.collectAsStateWithLifecycle()
    val totalBytes = regions.sumOf { it.sizeBytes }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    var renaming by remember { mutableStateOf<OfflineRegionInfo?>(null) }
    var confirmClearCache by remember { mutableStateOf(false) }
    val deletedMessage = stringResource(R.string.offline_deleted_snackbar)
    val undoLabel = stringResource(R.string.offline_undo)

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.offline_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.offline_back))
                    }
                },
                actions = {
                    IconButton(onClick = { confirmClearCache = true }) {
                        Icon(Icons.Rounded.DeleteSweep, stringResource(R.string.offline_clear_cache))
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
                        // Delete is staged, not immediate: the region hides, and the
                        // snackbar's Undo restores it untouched; letting it lapse commits.
                        onDelete = {
                            viewModel.stageDelete(region.id)
                            scope.launch {
                                val result = snackbar.showSnackbar(
                                    message = deletedMessage.format(region.name),
                                    actionLabel = undoLabel,
                                    duration = androidx.compose.material3.SnackbarDuration.Short,
                                )
                                if (result == SnackbarResult.ActionPerformed) {
                                    viewModel.undoDelete(region.id)
                                } else {
                                    viewModel.commitDelete(region.id)
                                }
                            }
                        },
                        onRename = { renaming = region },
                        onRetry = { viewModel.retry(region.id) },
                        onPause = { viewModel.pause(region.id) },
                        onResume = { viewModel.resume(region.id) },
                    )
                }
                item { Spacer(Modifier.height(16.dp)) }
            }
        }
    }

    renaming?.let { target ->
        NameInputDialog(
            title = stringResource(R.string.offline_rename_title),
            confirmLabel = stringResource(R.string.offline_rename_confirm),
            initial = target.name,
            onConfirm = { viewModel.rename(target.id, it); renaming = null },
            onDismiss = { renaming = null },
        )
    }
    if (confirmClearCache) {
        TurboConfirmDialog(
            title = stringResource(R.string.offline_clear_cache_title),
            body = stringResource(R.string.offline_clear_cache_body),
            confirmLabel = stringResource(R.string.offline_clear_cache_confirm),
            icon = Icons.Rounded.DeleteSweep,
            onConfirm = { viewModel.clearCache(); confirmClearCache = false },
            onDismiss = { confirmClearCache = false },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun RegionCard(
    region: OfflineRegionInfo,
    onDelete: () -> Unit,
    onRename: () -> Unit,
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
                    modifier = Modifier.clickable(onClick = onRename),
                )
                // The base map (+ any overlays) differentiates same-named regions and
                // shows what's actually available offline (e.g. avalanche terrain).
                Text(
                    region.base.title + region.overlays.takeIf { it.isNotEmpty() }
                        ?.joinToString(prefix = " · ") { it.title }.orEmpty(),
                    style = MaterialTheme.typography.labelSmall,
                    color = cs.onSurfaceVariant,
                )
                Spacer(Modifier.height(6.dp))
                when (region.status) {
                    OfflineStatus.Complete -> StatusLine(
                        icon = Icons.Rounded.DownloadDone,
                        tint = cs.primary,
                        text = stringResource(R.string.offline_downloaded, formatSize(region.sizeBytes)) +
                            region.createdAtEpochMs.takeIf { it > 0 }
                                ?.let { " · " + DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(it)) }
                                .orEmpty(),
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
