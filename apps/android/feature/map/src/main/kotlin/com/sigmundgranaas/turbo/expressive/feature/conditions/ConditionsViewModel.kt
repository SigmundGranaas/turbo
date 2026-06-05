package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

sealed interface ConditionsUiState {
    data object Loading : ConditionsUiState
    data class Content(val conditions: Conditions) : ConditionsUiState
    data object Error : ConditionsUiState
}

/** Loads live conditions (MET weather + Varsom danger) for a point on demand. */
@HiltViewModel
class ConditionsViewModel @Inject constructor(
    private val repository: ConditionsRepository,
) : ViewModel() {
    private val _state = MutableStateFlow<ConditionsUiState>(ConditionsUiState.Loading)
    val state: StateFlow<ConditionsUiState> = _state.asStateFlow()

    private var loadedFor: LatLng? = null

    fun load(point: LatLng) {
        if (loadedFor == point) return
        loadedFor = point
        _state.value = ConditionsUiState.Loading
        viewModelScope.launch {
            _state.value = when (val outcome = repository.forPoint(point)) {
                is Outcome.Success -> ConditionsUiState.Content(outcome.value)
                is Outcome.Failure -> ConditionsUiState.Error
            }
        }
    }
}
