package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.sigmundgranaas.turbo.expressive.domain.RecentSearch
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/** Persisted, most-recent-first list of places the user picked from search. */
interface RecentSearchRepository {
    val recents: Flow<List<RecentSearch>>
    suspend fun record(item: RecentSearch)
    suspend fun clear()
}

private val Context.recentSearchStore: DataStore<Preferences> by preferencesDataStore(name = "recent_searches")

@Singleton
class DataStoreRecentSearchRepository @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : RecentSearchRepository {

    private val key = stringPreferencesKey("recents_json")
    private val json = Json { ignoreUnknownKeys = true }

    override val recents: Flow<List<RecentSearch>> = context.recentSearchStore.data.map { prefs ->
        prefs[key]?.let { runCatching { json.decodeFromString<List<RecentDto>>(it) }.getOrNull() }
            ?.map { it.toDomain() }
            .orEmpty()
    }

    override suspend fun record(item: RecentSearch) {
        context.recentSearchStore.edit { prefs ->
            val current = prefs[key]?.let { runCatching { json.decodeFromString<List<RecentDto>>(it) }.getOrNull() }.orEmpty()
            // De-dupe by name + ~11 m rounded position, newest first, capped.
            val deduped = current.filterNot { it.sameAs(item) }
            val next = (listOf(RecentDto.from(item)) + deduped).take(MAX)
            prefs[key] = json.encodeToString(next)
        }
    }

    override suspend fun clear() {
        context.recentSearchStore.edit { it.remove(key) }
    }

    private companion object {
        const val MAX = 8
    }
}

@Serializable
private data class RecentDto(
    @SerialName("n") val name: String,
    @SerialName("s") val sub: String,
    @SerialName("lat") val lat: Double,
    @SerialName("lng") val lng: Double,
) {
    fun toDomain() = RecentSearch(name, sub, lat, lng)

    fun sameAs(item: RecentSearch): Boolean =
        name.equals(item.name, ignoreCase = true) &&
            "%.4f".format(lat) == "%.4f".format(item.lat) &&
            "%.4f".format(lng) == "%.4f".format(item.lng)

    companion object {
        fun from(item: RecentSearch) = RecentDto(item.name, item.sub, item.lat, item.lng)
    }
}
