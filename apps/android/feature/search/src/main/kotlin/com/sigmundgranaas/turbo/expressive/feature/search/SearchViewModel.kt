package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SearchResult(val name: String, val sub: String, val kind: ActivityKindId)

data class SearchUiState(
    val query: String = "Stor",
    val filter: Int = 0,
    val results: List<SearchResult> = DemoResults,
) {
    companion object {
        val DemoResults = listOf(
            SearchResult("Storsteinen", "Tromsø · Mountain · 421 m", ActivityKindId.Mountain),
            SearchResult("Sjurfjellet Hytte", "DNT cabin · Lyngen", ActivityKindId.Cabin),
            SearchResult("Storesandvika", "Beach · Senja", ActivityKindId.Beach),
            SearchResult("Stor-Bjørnen", "Viewpoint · Senja", ActivityKindId.Viewpoint),
        )
    }
}

/**
 * Search state holder, backed by the live Kartverket [SearchRepository]. Falls
 * back to the demo results if the network call fails, so the UI is never empty.
 */
@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: SearchRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()

    init {
        search(_state.value.query)
    }

    fun setFilter(index: Int) = _state.update { it.copy(filter = index) }

    fun search(query: String) {
        viewModelScope.launch {
            when (val outcome = repository.search(query)) {
                is Outcome.Success -> {
                    val hits = outcome.value
                    if (hits.isNotEmpty()) {
                        _state.update { s ->
                            s.copy(results = hits.map { SearchResult(it.name, it.description, ActivityKindId.Mountain) })
                        }
                    }
                }
                is Outcome.Failure -> Unit // keep the demo fallback
            }
        }
    }
}
