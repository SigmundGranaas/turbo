package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Loads the signed-in user's friend code for the account screen. */
@HiltViewModel
class SharingViewModel @Inject constructor(
    private val sharing: SharingRepository,
) : ViewModel() {

    /** The "turbo-XXXX" code, or null until loaded / on failure. */
    var friendCode by mutableStateOf<String?>(null)
        private set

    init {
        viewModelScope.launch {
            (sharing.friendCode() as? Outcome.Success)?.let { friendCode = it.value }
        }
    }
}
