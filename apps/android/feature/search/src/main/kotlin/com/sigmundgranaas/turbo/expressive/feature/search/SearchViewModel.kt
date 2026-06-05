package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

enum class SearchResultType { Coordinate, Marker, Place }

data class SearchResult(
    val name: String,
    val sub: String,
    val kind: ActivityKindId,
    val type: SearchResultType,
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
 * Search state holder. Each keystroke schedules a debounced lookup that fuses three
 * sources: a parsed lat/lng coordinate (if the query is one), the user's local
 * markers (matched by name), and live Kartverket place names. The filter chips
 * (All / Markers / Places) actually narrow the surfaced list; a coordinate hit is
 * always shown since it's the most direct intent.
 */
@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: SearchRepository,
    private val markerRepository: MarkerRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()

    private val markers = MutableStateFlow<List<Marker>>(emptyList())
    private var allResults: List<SearchResult> = emptyList()
    private var searchJob: Job? = null

    init {
        viewModelScope.launch { markerRepository.observeAll().collect { markers.value = it } }
    }

    fun setFilter(index: Int) {
        _state.update { it.copy(filter = index, results = applyFilter(allResults, index)) }
    }

    fun setQuery(query: String) {
        _state.update { it.copy(query = query) }
        searchJob?.cancel()
        if (query.isBlank()) {
            allResults = emptyList()
            _state.update { it.copy(results = emptyList(), loading = false) }
            return
        }

        // Synchronous hits (coordinate + local markers) appear immediately; the
        // network places stream in once the debounce elapses.
        val instant = instantResults(query)
        publish(instant, loading = true)

        searchJob = viewModelScope.launch {
            kotlinx.coroutines.delay(DEBOUNCE_MS)
            val places = when (val outcome = repository.search(query)) {
                is Outcome.Success -> outcome.value.map {
                    SearchResult(it.name, it.description, ActivityKindId.Mountain, SearchResultType.Place, it.position.lat, it.position.lng)
                }
                is Outcome.Failure -> emptyList()
            }
            publish(instant + places, loading = false)
        }
    }

    private fun instantResults(query: String): List<SearchResult> {
        val coordinate = parseCoordinate(query)?.let {
            listOf(SearchResult("Go to coordinate", formatCoords(it), ActivityKindId.Viewpoint, SearchResultType.Coordinate, it.lat, it.lng))
        }.orEmpty()
        val markerHits = markers.value
            .filter { it.name.contains(query.trim(), ignoreCase = true) }
            .map { SearchResult(it.name, "Saved marker · ${it.kind.label}", it.kind, SearchResultType.Marker, it.position.lat, it.position.lng) }
        return coordinate + markerHits
    }

    private fun publish(results: List<SearchResult>, loading: Boolean) {
        allResults = results
        _state.update { it.copy(loading = loading, results = applyFilter(results, it.filter)) }
    }

    private fun applyFilter(results: List<SearchResult>, filter: Int): List<SearchResult> = results.filter {
        when (filter) {
            FILTER_MARKERS -> it.type == SearchResultType.Marker || it.type == SearchResultType.Coordinate
            FILTER_PLACES -> it.type == SearchResultType.Place || it.type == SearchResultType.Coordinate
            else -> true
        }
    }

    private companion object {
        const val DEBOUNCE_MS = 280L
        const val FILTER_MARKERS = 1
        const val FILTER_PLACES = 2
    }
}

/**
 * Parse "lat, lng" / "lat lng" (comma or whitespace separated, optional sign) into a
 * [LatLng], rejecting out-of-range values. Returns null when the text isn't a coordinate.
 */
internal fun parseCoordinate(query: String): LatLng? {
    val match = Regex("""^\s*(-?\d{1,3}(?:\.\d+)?)\s*[,;\s]\s*(-?\d{1,3}(?:\.\d+)?)\s*$""").find(query) ?: return null
    val lat = match.groupValues[1].toDoubleOrNull() ?: return null
    val lng = match.groupValues[2].toDoubleOrNull() ?: return null
    if (lat !in -90.0..90.0 || lng !in -180.0..180.0) return null
    return LatLng(lat, lng)
}
