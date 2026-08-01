package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.auth.Account
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Settings UI state is the persisted [UserSettings] (DataStore-backed). */
@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val repository: SettingsRepository,
    auth: AuthRepository,
    // Appended, not inserted. Existing call sites pass these positionally,
    // and a new parameter in the middle silently re-binds every one of
    // them to the wrong argument.
    private val routeDiagnostics: com.sigmundgranaas.turbo.expressive.core.data.RouteDiagnostics =
        com.sigmundgranaas.turbo.expressive.core.data.RouteDiagnostics(),
    private val bundledPack: com.sigmundgranaas.turbo.expressive.core.data.BundledRoutingPack? = null,
) : ViewModel() {

    val state: StateFlow<UserSettings> = repository.settings.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = UserSettings(),
    )

    /** The signed-in account for the header, or null when signed out — the REAL
     *  identity from [AuthRepository] (this header used to be hardcoded). */
    val account: StateFlow<Account?> = auth.state
        .map { (it as? AuthState.SignedIn)?.account }
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(5_000),
            initialValue = null,
        )

    fun setCompass(enabled: Boolean) = viewModelScope.launch { repository.setCompassOrientation(enabled) }
    fun setFollow(enabled: Boolean) = viewModelScope.launch { repository.setFollowLocation(enabled) }
    fun setMetric(metric: Boolean) = viewModelScope.launch { repository.setMetricUnits(metric) }
    fun setThemeMode(mode: ThemeMode) = viewModelScope.launch { repository.setThemeMode(mode) }
    fun setCloudSync(enabled: Boolean) = viewModelScope.launch { repository.setCloudSyncEnabled(enabled) }
    fun setWifiOnly(enabled: Boolean) = viewModelScope.launch { repository.setDownloadOverWifiOnly(enabled) }
    fun setLocationDotColor(colorHex: String?) = viewModelScope.launch { repository.setLocationDotColor(colorHex) }
    fun setShowHeadingBeam(enabled: Boolean) = viewModelScope.launch { repository.setShowHeadingBeam(enabled) }
    fun setGestures(gestures: com.sigmundgranaas.turbo.expressive.domain.GestureSettings) =
        viewModelScope.launch { repository.setGestures(gestures) }
    /**
     * The last few solves, for the routing section. Read-only and in
     * memory — see `RouteDiagnostics`.
     */
    val routeSolves = routeDiagnostics.records

    fun setRouteEngine(engine: com.sigmundgranaas.turbo.expressive.domain.RouteEngine) =
        viewModelScope.launch { repository.setRouteEngine(engine) }

    fun clearRouteSolves() = routeDiagnostics.clear()

    /** The APK's own pack: what it is, how big, and whether it is unpacked. */
    data class BundledPackState(
        val description: String,
        val sizeBytes: Long,
        val installed: Boolean,
        val busy: Boolean = false,
    )

    private val _bundledPack = MutableStateFlow(bundledPack?.let {
        BundledPackState(it.description, it.sizeBytes, it.isInstalled())
    })
    val bundledPackState: StateFlow<BundledPackState?> = _bundledPack.asStateFlow()

    fun installBundledPack() {
        val pack = bundledPack ?: return
        val current = _bundledPack.value ?: return
        if (current.busy) return
        _bundledPack.value = current.copy(busy = true)
        viewModelScope.launch {
            if (current.installed) pack.uninstall() else pack.install()
            _bundledPack.value = current.copy(installed = pack.isInstalled(), busy = false)
        }
    }

    fun setExperimentalTrails(enabled: Boolean) = viewModelScope.launch { repository.setExperimentalTrails(enabled) }
    fun setExperimentalClouds(enabled: Boolean) = viewModelScope.launch { repository.setExperimentalClouds(enabled) }
    fun setRotationLocked(enabled: Boolean) = viewModelScope.launch { repository.setRotationLocked(enabled) }
}
