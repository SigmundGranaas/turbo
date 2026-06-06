package com.sigmundgranaas.turbo.expressive.feature.collections

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

private val CollectionColors = listOf(0xFF8F4C38L, 0xFF1A73E8L, 0xFF2E7D32L, 0xFFE0432BL, 0xFF6A4FB3L, null)

/** Local collections: a list of folders with colour + membership count; create/edit/delete. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CollectionsScreen(
    onBack: () -> Unit,
    viewModel: CollectionsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val collections by viewModel.collections.collectAsStateWithLifecycle()
    var editing by remember { mutableStateOf<MapCollection?>(null) }
    var showEditor by remember { mutableStateOf(false) }

    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.collections_title), style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.collections_back)) } },
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = { editing = null; showEditor = true }) {
                Icon(Icons.Rounded.Add, stringResource(R.string.collections_new))
            }
        },
    ) { pad ->
        if (collections.isEmpty()) {
            EmptyState(
                icon = Icons.Rounded.Folder,
                title = stringResource(R.string.collections_empty_title),
                body = stringResource(R.string.collections_empty_body),
                modifier = Modifier.fillMaxSize().padding(pad),
            )
        } else {
            LazyColumn(
                Modifier.fillMaxHeight().padding(pad).responsiveContentWidth().padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item { Spacer(Modifier.size(4.dp)) }
                items(collections.size) { i ->
                    val c = collections[i]
                    CollectionRow(
                        collection = c,
                        onEdit = { editing = c; showEditor = true },
                        onDelete = { viewModel.delete(c.id) },
                    )
                }
                item { Spacer(Modifier.size(24.dp)) }
            }
        }
    }

    if (showEditor) {
        CollectionEditorDialog(
            existing = editing,
            onDismiss = { showEditor = false },
            onSave = { name, color -> viewModel.upsert(editing?.id, name, color); showEditor = false },
        )
    }
}

@Composable
private fun CollectionRow(collection: MapCollection, onEdit: () -> Unit, onDelete: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val accent = collection.colorArgb?.let { Color(it) } ?: cs.primary
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)).background(cs.surfaceContainerHigh)
            .clickable(onClick = onEdit).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(TurboRadius.m)).background(accent.copy(alpha = 0.2f)), contentAlignment = Alignment.Center) {
            Icon(Icons.Rounded.Folder, null, tint = accent, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.size(12.dp))
        Column(Modifier.weight(1f)) {
            Text(collection.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
            Text(
                if (collection.itemCount == 1) {
                    stringResource(R.string.collections_item_count_one)
                } else {
                    stringResource(R.string.collections_item_count_other, collection.itemCount)
                },
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant,
            )
        }
        IconButton(onClick = onDelete) {
            Icon(Icons.Rounded.DeleteOutline, stringResource(R.string.collections_delete), tint = cs.onSurfaceVariant)
        }
    }
}

@Composable
private fun CollectionEditorDialog(
    existing: MapCollection?,
    onDismiss: () -> Unit,
    onSave: (name: String, colorArgb: Long?) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var name by remember { mutableStateOf(existing?.name.orEmpty()) }
    var color by remember { mutableStateOf(existing?.colorArgb) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(if (existing == null) R.string.collections_new else R.string.collections_edit)) },
        text = {
            Column {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.collections_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.size(16.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    CollectionColors.forEach { swatch ->
                        val swatchColor = swatch?.let { Color(it) } ?: cs.primary
                        Box(
                            Modifier.size(32.dp).clip(CircleShape).background(swatchColor)
                                .border(if (swatch == color) 3.dp else 0.dp, cs.onSurface, CircleShape)
                                .clickable { color = swatch },
                        )
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSave(name, color) }) { Text(stringResource(R.string.collections_save)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.collections_cancel)) } },
    )
}
