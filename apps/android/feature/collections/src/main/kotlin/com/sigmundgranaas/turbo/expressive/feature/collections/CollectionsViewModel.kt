package com.sigmundgranaas.turbo.expressive.feature.collections

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/** Lists user collections and supports create / rename / recolour / delete. */
@HiltViewModel
class CollectionsViewModel @Inject constructor(
    private val repository: CollectionRepository,
) : ViewModel() {

    val collections: StateFlow<List<MapCollection>> = repository.observeAll().stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = emptyList(),
    )

    /** Create a new collection, or rename/recolour an existing one (same id). */
    fun upsert(id: String?, name: String, colorArgb: Long?) {
        val safeName = name.trim().ifBlank { "Collection" }
        viewModelScope.launch {
            repository.upsert(
                MapCollection(
                    id = id ?: "c-${UUID.randomUUID()}",
                    name = safeName,
                    colorArgb = colorArgb,
                ),
            )
        }
    }

    fun delete(id: String) = viewModelScope.launch { repository.delete(id) }
}
