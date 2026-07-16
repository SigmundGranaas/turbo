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
import kotlinx.coroutines.flow.SharingStarted
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
}
