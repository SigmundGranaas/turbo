package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

private val Context.syncCursorStore: DataStore<Preferences> by preferencesDataStore(name = "turbo_sync_cursors")

/**
 * Per-entity delta-sync cursor: the server `serverTime` returned by the last
 * successful pull, replayed as `?since=` on the next pull. One opaque string per
 * sync domain (e.g. "tracks", "geo", "collections"); null until the first sync.
 */
interface SyncCursorStore {
    suspend fun cursor(entity: String): String?
    suspend fun setCursor(entity: String, serverTime: String)
    /** Wipe all cursors — used on sign-out so a new account re-pulls from scratch. */
    suspend fun clear()
}

@Singleton
class DataStoreSyncCursorStore @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : SyncCursorStore {

    override suspend fun cursor(entity: String): String? =
        context.syncCursorStore.data.first()[stringPreferencesKey(entity)]

    override suspend fun setCursor(entity: String, serverTime: String) {
        context.syncCursorStore.edit { it[stringPreferencesKey(entity)] = serverTime }
    }

    override suspend fun clear() {
        context.syncCursorStore.edit { it.clear() }
    }
}
