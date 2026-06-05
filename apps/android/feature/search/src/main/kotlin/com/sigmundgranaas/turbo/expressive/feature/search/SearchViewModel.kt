package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SearchResult(
    val name: String,
    val sub: String,
    val kind: ActivityKindId,
    val lat: Double? = null,
    val lng: Double? = null,
)

data class SearchUiState(
    val query: String = "",
    val filter: Int = 0,
    val loading: Boolean = false,
    val results: List<SearchResult> = emptyList(),
)

/**
 * Search state holder backed by the live Kartverket [SearchRepository]. The query
 * is fully editable; each keystroke schedules a debounced lookup. Results carry
 * coordinates so the caller can fly the map to the chosen place.
 */
@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: SearchRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()

    private var searchJob: Job? = null

    fun setFilter(index: Int) = _state.update { it.copy(filter = index) }

    fun setQuery(query: String) {
        _state.update { it.copy(query = query) }
        searchJob?.cancel()
        if (query.isBlank()) {
            _state.update { it.copy(results = emptyList(), loading = false) }
            return
        }
        searchJob = viewModelScope.launch {
            delay(DEBOUNCE_MS)
            _state.update { it.copy(loading = true) }
            when (val outcome = repository.search(query)) {
                is Outcome.Success -> _state.update { s ->
                    s.copy(
                        loading = false,
                        results = outcome.value.map {
                            SearchResult(it.name, it.description, ActivityKindId.Mountain, it.position.lat, it.position.lng)
                        },
                    )
                }
                is Outcome.Failure -> _state.update { it.copy(loading = false, results = emptyList()) }
            }
        }
    }

    private companion object {
        const val DEBOUNCE_MS = 280L
    }
}
