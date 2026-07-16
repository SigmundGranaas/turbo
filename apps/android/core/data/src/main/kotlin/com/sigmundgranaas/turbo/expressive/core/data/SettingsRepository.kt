package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.CustomTileSource
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
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
    suspend fun setThemeMode(mode: ThemeMode)
    suspend fun setCloudSyncEnabled(enabled: Boolean)
    suspend fun setDownloadOverWifiOnly(enabled: Boolean)
    suspend fun setBaseLayer(layer: BaseLayer)

    /** My-position dot colour ("#RRGGBB"); null restores the default blue. */
    suspend fun setLocationDotColor(colorHex: String?)

    /** Show/hide the my-position heading beam. */
    suspend fun setShowHeadingBeam(enabled: Boolean)

    /** Add a user-supplied XYZ basemap and make it the active base. */
    suspend fun addCustomTileSource(source: CustomTileSource)

    /** Remove a custom basemap; if it was active, fall back to the built-in [UserSettings.baseLayer]. */
    suspend fun removeCustomTileSource(id: String)

    /** Select a custom basemap by id (null = back to the built-in base layer). */
    suspend fun selectCustomTileSource(id: String?)

    /** Persist the map camera so reopening the app returns to where the user left it. */
    suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double)
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
        val THEME_MODE = stringPreferencesKey("theme_mode")
        val CLOUD_SYNC = booleanPreferencesKey("cloud_sync_enabled")
        val WIFI_ONLY = booleanPreferencesKey("download_wifi_only")
        val BASE_LAYER = stringPreferencesKey("base_layer")
        val CAM_LAT = doublePreferencesKey("last_camera_lat")
        val CAM_LNG = doublePreferencesKey("last_camera_lng")
        val CAM_ZOOM = doublePreferencesKey("last_camera_zoom")
        val LOCATION_DOT_COLOR = stringPreferencesKey("location_dot_color")
        val HEADING_BEAM = booleanPreferencesKey("show_heading_beam")
        val CUSTOM_SOURCES = stringPreferencesKey("custom_tile_sources")
        val CUSTOM_SELECTED = stringPreferencesKey("custom_tile_source_selected")
    }

    override val settings: Flow<UserSettings> = context.settingsDataStore.data.map { prefs ->
        UserSettings(
            compassOrientation = prefs[Keys.COMPASS] ?: true,
            followLocation = prefs[Keys.FOLLOW] ?: false,
            metricUnits = prefs[Keys.METRIC] ?: true,
            themeMode = prefs[Keys.THEME_MODE]
                ?.let { runCatching { ThemeMode.valueOf(it) }.getOrNull() }
                ?: ThemeMode.System,
            cloudSyncEnabled = prefs[Keys.CLOUD_SYNC] ?: true,
            downloadOverWifiOnly = prefs[Keys.WIFI_ONLY] ?: false,
            baseLayer = prefs[Keys.BASE_LAYER]
                ?.let { id -> BaseLayer.entries.firstOrNull { it.id == id } }
                ?: BaseLayer.Norgeskart,
            lastCameraLat = prefs[Keys.CAM_LAT],
            lastCameraLng = prefs[Keys.CAM_LNG],
            lastCameraZoom = prefs[Keys.CAM_ZOOM],
            locationDotColorHex = prefs[Keys.LOCATION_DOT_COLOR],
            showHeadingBeam = prefs[Keys.HEADING_BEAM] ?: true,
            customTileSources = decodeCustomSources(prefs[Keys.CUSTOM_SOURCES]),
            selectedCustomSourceId = prefs[Keys.CUSTOM_SELECTED],
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

    override suspend fun setThemeMode(mode: ThemeMode) {
        context.settingsDataStore.edit { it[Keys.THEME_MODE] = mode.name }
    }

    override suspend fun setCloudSyncEnabled(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.CLOUD_SYNC] = enabled }
    }

    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.WIFI_ONLY] = enabled }
    }

    override suspend fun setBaseLayer(layer: BaseLayer) {
        context.settingsDataStore.edit {
            it[Keys.BASE_LAYER] = layer.id
            // Picking a built-in releases any active custom basemap.
            it.remove(Keys.CUSTOM_SELECTED)
        }
    }

    override suspend fun addCustomTileSource(source: CustomTileSource) {
        context.settingsDataStore.edit { prefs ->
            val next = decodeCustomSources(prefs[Keys.CUSTOM_SOURCES]).filterNot { it.id == source.id } + source
            prefs[Keys.CUSTOM_SOURCES] = encodeCustomSources(next)
            prefs[Keys.CUSTOM_SELECTED] = source.id
        }
    }

    override suspend fun removeCustomTileSource(id: String) {
        context.settingsDataStore.edit { prefs ->
            val next = decodeCustomSources(prefs[Keys.CUSTOM_SOURCES]).filterNot { it.id == id }
            prefs[Keys.CUSTOM_SOURCES] = encodeCustomSources(next)
            if (prefs[Keys.CUSTOM_SELECTED] == id) prefs.remove(Keys.CUSTOM_SELECTED)
        }
    }

    override suspend fun selectCustomTileSource(id: String?) {
        context.settingsDataStore.edit {
            if (id == null) it.remove(Keys.CUSTOM_SELECTED) else it[Keys.CUSTOM_SELECTED] = id
        }
    }

    override suspend fun setLocationDotColor(colorHex: String?) {
        context.settingsDataStore.edit {
            if (colorHex == null) it.remove(Keys.LOCATION_DOT_COLOR) else it[Keys.LOCATION_DOT_COLOR] = colorHex
        }
    }

    override suspend fun setShowHeadingBeam(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.HEADING_BEAM] = enabled }
    }

    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) {
        context.settingsDataStore.edit {
            it[Keys.CAM_LAT] = lat
            it[Keys.CAM_LNG] = lng
            it[Keys.CAM_ZOOM] = zoom
        }
    }
}

/** Persistence shape for one custom basemap (JSON list under one preference key). */
@Serializable
private data class CustomSourceDto(
    val id: String,
    val name: String,
    val urlTemplate: String,
    val maxZoom: Int = CustomTileSource.DEFAULT_MAX_ZOOM,
)

private val customSourcesJson = Json { ignoreUnknownKeys = true }

private fun encodeCustomSources(sources: List<CustomTileSource>): String =
    customSourcesJson.encodeToString(sources.map { CustomSourceDto(it.id, it.name, it.urlTemplate, it.maxZoom) })

private fun decodeCustomSources(encoded: String?): List<CustomTileSource> =
    encoded?.takeIf { it.isNotBlank() }?.let { json ->
        runCatching {
            customSourcesJson.decodeFromString<List<CustomSourceDto>>(json)
                .map { CustomTileSource(it.id, it.name, it.urlTemplate, it.maxZoom) }
        }.getOrDefault(emptyList())
    } ?: emptyList()
