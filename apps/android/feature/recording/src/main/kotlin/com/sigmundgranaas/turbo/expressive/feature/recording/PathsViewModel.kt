package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/** Lists persisted tracks/routes and supports deleting them. */
@HiltViewModel
class PathsViewModel @Inject constructor(
    private val repository: PathRepository,
) : ViewModel() {
    val paths: StateFlow<List<SavedPath>> = repository.observeAll().stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = emptyList(),
    )

    fun delete(id: String) = viewModelScope.launch { repository.delete(id) }

    /** Persist an imported track, naming it from the file when the track itself is unnamed. */
    fun importTrack(parsed: ParsedTrack, fallbackName: String) = viewModelScope.launch {
        repository.save(
            SavedPath(
                id = "p-${UUID.randomUUID()}",
                name = parsed.name?.takeIf { it.isNotBlank() } ?: fallbackName,
                path = parsed.geo,
            ),
        )
    }
}
