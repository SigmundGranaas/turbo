package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ElevationRepository
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
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
    private val sharing: SharingRepository,
    private val elevations: ElevationRepository,
) : ViewModel() {
    val paths: StateFlow<List<SavedPath>> = repository.observeAll().stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = emptyList(),
    )

    fun delete(id: String) = viewModelScope.launch { repository.delete(id) }

    /** Outcome of requesting a cloud share link for a track. */
    sealed interface ShareLinkResult {
        data class Ready(val url: String) : ShareLinkResult
        /** The track hasn't been uploaded yet — sign in / sync first, then share. */
        data object NotSynced : ShareLinkResult
        data object Failed : ShareLinkResult
    }

    /** Create a cloud share link for a synced track and hand the shareable URL back to the UI. */
    fun createShareLink(pathId: String, onResult: (ShareLinkResult) -> Unit) = viewModelScope.launch {
        val remoteId = repository.remoteId(pathId)
        if (remoteId == null) {
            onResult(ShareLinkResult.NotSynced)
            return@launch
        }
        onResult(
            when (val out = sharing.createLink(remoteId)) {
                is Outcome.Success -> ShareLinkResult.Ready(out.value)
                is Outcome.Failure -> ShareLinkResult.Failed
            },
        )
    }

    /** Set (or clear with null) the track's display colour ("#RRGGBB"). Synced as
     *  `colorHex`, so the choice renders on every client, not just this device. */
    fun setColor(id: String, colorHex: String?) = viewModelScope.launch {
        val existing = repository.byId(id) ?: return@launch
        repository.save(existing.copy(colorHex = colorHex))
    }

    /** Rename a track in place (save upserts by id), keeping its geometry + stats. */
    fun rename(id: String, name: String) = viewModelScope.launch {
        val trimmed = name.trim()
        if (trimmed.isEmpty()) return@launch
        val existing = repository.byId(id) ?: return@launch
        repository.save(existing.copy(name = trimmed))
    }

    /** Persist an imported track, naming it from the file when the track itself is unnamed.
     *  Files without per-point elevation get theirs backfilled from the tileserver DEM
     *  first (best-effort), so the elevation chart + ascent/descent work on import. */
    fun importTrack(parsed: ParsedTrack, fallbackName: String) = viewModelScope.launch {
        repository.save(
            SavedPath(
                id = "p-${UUID.randomUUID()}",
                name = parsed.name?.takeIf { it.isNotBlank() } ?: fallbackName,
                path = withBackfilledElevations(parsed.geo),
            ),
        )
    }

    /**
     * Fill missing per-point elevations from the DEM when the imported file lacked
     * them (≥ half missing — a file that mostly HAS elevation keeps its own data),
     * then recompute ascent/descent from the merged series. Best-effort: transport
     * failure or an oversized track imports unchanged rather than failing the import.
     */
    private suspend fun withBackfilledElevations(geo: GeoPath): GeoPath {
        val points = geo.points
        if (points.size < 2 || points.size > MAX_BACKFILL_POINTS) return geo
        val existing = geo.elevations
        val missing = existing?.count { it == null } ?: points.size
        if (missing * 2 < points.size) return geo
        val sampled = when (val out = elevations.sample(points)) {
            is Outcome.Success -> out.value
            is Outcome.Failure -> return geo
        }
        val merged = points.indices.map { i -> existing?.getOrNull(i) ?: sampled.getOrNull(i) }
        if (merged.none { it != null }) return geo
        val (asc, desc) = GeoMetrics.gainLoss(merged)
        return geo.copy(
            elevations = merged,
            ascentM = asc ?: geo.ascentM,
            descentM = desc ?: geo.descentM,
        )
    }

    private companion object {
        /** One request chunk on the server; larger imports skip backfill (rare, huge files). */
        const val MAX_BACKFILL_POINTS = 4096
    }
}
