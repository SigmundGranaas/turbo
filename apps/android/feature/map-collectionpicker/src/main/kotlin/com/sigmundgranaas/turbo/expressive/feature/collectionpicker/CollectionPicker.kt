package com.sigmundgranaas.turbo.expressive.feature.collectionpicker

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/** Lists collections and toggles a given item's membership; can create new ones. */
@HiltViewModel
class CollectionPickerViewModel @Inject constructor(
    private val repository: CollectionRepository,
) : ViewModel() {
    val collections: StateFlow<List<MapCollection>> = repository.observeAll()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun membership(itemId: String, type: CollectionItemType): StateFlow<List<String>> =
        repository.observeCollectionsForItem(itemId, type)
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun toggle(collectionId: String, itemId: String, type: CollectionItemType, member: Boolean) {
        viewModelScope.launch {
            if (member) repository.addItem(collectionId, itemId, type) else repository.removeItem(collectionId, itemId, type)
        }
    }

    fun create(name: String, itemId: String, type: CollectionItemType) {
        val id = "c-${UUID.randomUUID()}"
        viewModelScope.launch {
            repository.upsert(MapCollection(id = id, name = name.trim().ifBlank { "Collection" }))
            repository.addItem(id, itemId, type)
        }
    }
}

/** Bottom sheet to add/remove [itemId] (a marker/track) to/from collections. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CollectionPickerSheet(
    itemId: String,
    type: CollectionItemType,
    onDismiss: () -> Unit,
    viewModel: CollectionPickerViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val collections by viewModel.collections.collectAsStateWithLifecycle()
    val member by remember(itemId) { viewModel.membership(itemId, type) }.collectAsStateWithLifecycle()
    var newName by remember { mutableStateOf("") }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, bottom = 28.dp)) {
            Text(stringResource(R.string.collections_add_title), style = MaterialTheme.typography.headlineSmall, color = cs.onSurface)
            LazyColumn(Modifier.fillMaxWidth().padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(collections.size) { i ->
                    val c = collections[i]
                    val isMember = c.id in member
                    Row(
                        Modifier.fillMaxWidth().clickable { viewModel.toggle(c.id, itemId, type, !isMember) }
                            .padding(vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(c.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                        if (isMember) Icon(Icons.Rounded.Check, stringResource(R.string.collections_member), tint = cs.primary)
                    }
                }
                item {
                    Surface(shape = RoundedCornerShape(TurboRadius.m), color = cs.surfaceContainerHigh, modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 14.dp, end = 4.dp)) {
                            BasicTextField(
                                value = newName,
                                onValueChange = { newName = it },
                                singleLine = true,
                                textStyle = MaterialTheme.typography.bodyLarge.copy(color = cs.onSurface),
                                cursorBrush = SolidColor(cs.primary),
                                decorationBox = { inner ->
                                    if (newName.isEmpty()) Text(stringResource(R.string.collections_new_hint), color = cs.onSurfaceVariant)
                                    inner()
                                },
                                modifier = Modifier.weight(1f).padding(vertical = 14.dp),
                            )
                            IconButton(onClick = { if (newName.isNotBlank()) { viewModel.create(newName, itemId, type); newName = "" } }) {
                                Icon(Icons.Rounded.Add, stringResource(R.string.collections_create), tint = cs.primary)
                            }
                        }
                    }
                }
            }
        }
    }
}
