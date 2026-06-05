package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/** Persisted user settings, exposed as a [Flow] so the UI re-renders on change. */
interface SettingsRepository {
    val settings: Flow<UserSettings>
    suspend fun setCompassOrientation(enabled: Boolean)
    suspend fun setFollowLocation(enabled: Boolean)
    suspend fun setMetricUnits(metric: Boolean)
}

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "user_settings")

@Singleton
class DataStoreSettingsRepository @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : SettingsRepository {

    private object Keys {
        val COMPASS = booleanPreferencesKey("compass_orientation")
        val FOLLOW = booleanPreferencesKey("follow_location")
        val METRIC = booleanPreferencesKey("metric_units")
    }

    override val settings: Flow<UserSettings> = context.settingsDataStore.data.map { prefs ->
        UserSettings(
            compassOrientation = prefs[Keys.COMPASS] ?: true,
            followLocation = prefs[Keys.FOLLOW] ?: false,
            metricUnits = prefs[Keys.METRIC] ?: true,
        )
    }

    override suspend fun setCompassOrientation(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.COMPASS] = enabled }
    }

    override suspend fun setFollowLocation(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.FOLLOW] = enabled }
    }

    override suspend fun setMetricUnits(metric: Boolean) {
        context.settingsDataStore.edit { it[Keys.METRIC] = metric }
    }
}
