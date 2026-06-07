package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.sync.SyncController
import com.sigmundgranaas.turbo.expressive.core.sync.SyncStatus
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Surfaces sync status and a manual "sync now" trigger to the account screen. */
@HiltViewModel
class SyncViewModel @Inject constructor(
    private val controller: SyncController,
) : ViewModel() {

    val status: StateFlow<SyncStatus> = controller.status

    fun syncNow() {
        viewModelScope.launch { controller.syncNow() }
    }
}
