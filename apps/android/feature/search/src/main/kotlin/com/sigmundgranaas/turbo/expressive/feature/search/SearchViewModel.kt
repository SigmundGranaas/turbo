package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import com.sigmundgranaas.turbo.expressive.core.data.AddressSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.KommuneSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecentSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.TrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.RecentSearch
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

enum class SearchResultType { Coordinate, Marker, Place, Trail, Address, Kommune }

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
    val error: Boolean = false,
    val results: List<SearchResult> = emptyList(),
)

/**
 * Search state holder. Each keystroke schedules a debounced lookup that fuses the
 * sources: a parsed lat/lng coordinate (if the query is one), the user's local
 * markers (matched by name), and the live Kartverket backends — place names,
 * street addresses (Adresser) and municipalities (Kommuneinfo). The filter chips
 * (All / Markers / Places) actually narrow the surfaced list; a coordinate hit is
 * always shown since it's the most direct intent.
 */
@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: SearchRepository,
    private val markerRepository: MarkerRepository,
    private val recentSearchRepository: RecentSearchRepository,
    private val trailRepository: TrailSearchRepository,
    private val addressRepository: AddressSearchRepository,
    private val kommuneRepository: KommuneSearchRepository,
    private val strings: StringProvider,
) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()

    /** Most-recent-first picks, surfaced when the query is empty. */
    val recents: StateFlow<List<RecentSearch>> = recentSearchRepository.recents
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private val markers = MutableStateFlow<List<Marker>>(emptyList())
    private var allResults: List<SearchResult> = emptyList()
    private var searchJob: Job? = null

    init {
        viewModelScope.launch { markerRepository.observeAll().collect { markers.value = it } }
    }

    /** Record a chosen result so it surfaces in the recents list next time. */
    fun recordPick(result: SearchResult) {
        if (result.lat == null || result.lng == null) return
        viewModelScope.launch {
            recentSearchRepository.record(RecentSearch(result.name, result.sub, result.lat, result.lng))
        }
    }

    fun clearRecents() {
        viewModelScope.launch { recentSearchRepository.clear() }
    }

    fun setFilter(index: Int) {
        _state.update { it.copy(filter = index, results = applyFilter(allResults, index)) }
    }

    fun setQuery(query: String) {
        _state.update { it.copy(query = query) }
        searchJob?.cancel()
        if (query.isBlank()) {
            allResults = emptyList()
            _state.update { it.copy(results = emptyList(), loading = false, error = false) }
            return
        }

        // Synchronous hits (coordinate + local markers) appear immediately; the
        // network places stream in once the debounce elapses.
        val instant = instantResults(query)
        publish(instant, loading = true)

        searchJob = viewModelScope.launch {
            kotlinx.coroutines.delay(DEBOUNCE_MS)
            // Places (Kartverket stedsnavn), named trails (Nasjonalturbase), street
            // addresses (Adresser) and municipalities (Kommuneinfo) are independent
            // network sources — fan out concurrently and fuse.
            val placesDeferred = async { repository.search(query) }
            val trailsDeferred = async { trailRepository.search(query) }
            val addressesDeferred = async { addressRepository.search(query) }
            val kommunerDeferred = async { kommuneRepository.search(query) }
            val placesOutcome = placesDeferred.await()
            val trailsOutcome = trailsDeferred.await()
            val addressesOutcome = addressesDeferred.await()
            val kommunerOutcome = kommunerDeferred.await()

            val places = (placesOutcome as? Outcome.Success)?.value.orEmpty().map {
                SearchResult(it.name, it.description, ActivityKindId.Mountain, SearchResultType.Place, it.position.lat, it.position.lng)
            }
            val trails = (trailsOutcome as? Outcome.Success)?.value.orEmpty().map {
                SearchResult(it.name, it.description, ActivityKindId.Hiking, SearchResultType.Trail, it.position.lat, it.position.lng)
            }
            val addresses = (addressesOutcome as? Outcome.Success)?.value.orEmpty().map {
                SearchResult(it.name, it.description, ActivityKindId.Cabin, SearchResultType.Address, it.position.lat, it.position.lng)
            }
            val kommuner = (kommunerOutcome as? Outcome.Success)?.value.orEmpty().map {
                SearchResult(it.name, it.description, ActivityKindId.Park, SearchResultType.Kommune, it.position.lat, it.position.lng)
            }

            val outcomes = listOf(placesOutcome, trailsOutcome, addressesOutcome, kommunerOutcome)
            // Error only when EVERY network source fails; a partial failure still surfaces the rest.
            if (outcomes.all { it is Outcome.Failure }) {
                allResults = instant
                _state.update { it.copy(loading = false, error = true, results = applyFilter(instant, it.filter)) }
            } else {
                // Municipalities first (broadest intent), then places/trails, then addresses.
                publish(instant + kommuner + places + trails + addresses, loading = false)
            }
        }
    }

    /** Re-run the current query (used by the error-state Retry button). */
    fun retry() = setQuery(_state.value.query)

    private fun instantResults(query: String): List<SearchResult> {
        val coordinate = parseCoordinate(query)?.let {
            listOf(SearchResult(strings.get(R.string.search_go_to_coordinate), formatCoords(it), ActivityKindId.Viewpoint, SearchResultType.Coordinate, it.lat, it.lng))
        }.orEmpty()
        val markerHits = markers.value
            .filter { it.name.contains(query.trim(), ignoreCase = true) }
            .map { SearchResult(it.name, strings.get(R.string.search_saved_marker, strings.get(it.kind.labelRes)), it.kind, SearchResultType.Marker, it.position.lat, it.position.lng) }
        return coordinate + markerHits
    }

    private fun publish(results: List<SearchResult>, loading: Boolean) {
        allResults = results
        _state.update { it.copy(loading = loading, error = false, results = applyFilter(results, it.filter)) }
    }

    private fun applyFilter(results: List<SearchResult>, filter: Int): List<SearchResult> = results.filter {
        when (filter) {
            FILTER_MARKERS -> it.type == SearchResultType.Marker || it.type == SearchResultType.Coordinate
            // "Places" = everything geographic that isn't the user's own data:
            // place names, trails, street addresses and municipalities.
            FILTER_PLACES -> it.type != SearchResultType.Marker
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
