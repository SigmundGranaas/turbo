package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Settings UI state is the persisted [UserSettings] (DataStore-backed). */
@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val repository: SettingsRepository,
) : ViewModel() {

    val state: StateFlow<UserSettings> = repository.settings.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = UserSettings(),
    )

    fun setCompass(enabled: Boolean) = viewModelScope.launch { repository.setCompassOrientation(enabled) }
    fun setFollow(enabled: Boolean) = viewModelScope.launch { repository.setFollowLocation(enabled) }
    fun setMetric(metric: Boolean) = viewModelScope.launch { repository.setMetricUnits(metric) }
}
