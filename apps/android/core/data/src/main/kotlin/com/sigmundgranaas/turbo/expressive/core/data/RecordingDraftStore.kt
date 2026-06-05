package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/** A recording snapshot persisted across process death so a track survives a kill. */
data class RecordingDraft(val points: List<LatLng>, val elapsedSec: Int)

/**
 * Durable scratch store for the in-progress recording. The controller writes the
 * accumulated points + elapsed time as they grow and clears it on save/discard,
 * so a foreground-service restart (or process death) can resume the track.
 */
interface RecordingDraftStore {
    suspend fun load(): RecordingDraft?
    suspend fun save(points: List<LatLng>, elapsedSec: Int)
    suspend fun clear()
}

private val Context.recordingDraftStore: DataStore<Preferences> by preferencesDataStore(name = "recording_draft")

@Singleton
class DataStoreRecordingDraftStore @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : RecordingDraftStore {

    private object Keys {
        val POINTS = stringPreferencesKey("draft_points")
        val ELAPSED = intPreferencesKey("draft_elapsed")
    }

    override suspend fun load(): RecordingDraft? {
        val prefs = context.recordingDraftStore.data.first()
        val encoded = prefs[Keys.POINTS]?.takeIf { it.isNotBlank() } ?: return null
        val points = encoded.split(";").mapNotNull { pair ->
            val parts = pair.split(",")
            val lat = parts.getOrNull(0)?.toDoubleOrNull()
            val lng = parts.getOrNull(1)?.toDoubleOrNull()
            if (lat != null && lng != null) LatLng(lat, lng) else null
        }
        if (points.isEmpty()) return null
        return RecordingDraft(points, prefs[Keys.ELAPSED] ?: 0)
    }

    override suspend fun save(points: List<LatLng>, elapsedSec: Int) {
        context.recordingDraftStore.edit { prefs ->
            prefs[Keys.POINTS] = points.joinToString(";") { "${it.lat},${it.lng}" }
            prefs[Keys.ELAPSED] = elapsedSec
        }
    }

    override suspend fun clear() {
        context.recordingDraftStore.edit { it.clear() }
    }
}
