package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.data.SyncCursorStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject
import javax.inject.Singleton

/** One sync-able domain (tracks / markers / collections). */
interface DomainSyncer {
    /** Cursor key under which this domain's last serverTime is persisted. */
    val cursorKey: String

    /** Pull deltas since [since], merge, push local pending; return the new serverTime cursor (or null). */
    suspend fun sync(since: String?): String?
}

/** Observable engine state for a sync-status chip. */
sealed interface SyncStatus {
    data object Idle : SyncStatus
    data object Syncing : SyncStatus
    data class Failed(val domains: List<String>) : SyncStatus
}

/** Outcome of a single [SyncEngine.syncNow] run. */
sealed interface SyncOutcome {
    data object NotSignedIn : SyncOutcome
    data object Success : SyncOutcome
    data class PartialFailure(val errors: List<String>) : SyncOutcome
}

/**
 * Coordinates a full sync across every [DomainSyncer]: per-domain it reads the
 * persisted cursor, runs pull→merge→push, and stores the returned serverTime.
 * Serialized by a mutex so triggers can't overlap. Syncs automatically when the
 * user signs in and clears cursors on sign-out.
 */
@Singleton
class SyncEngine @Inject constructor(
    private val syncers: Set<@JvmSuppressWildcards DomainSyncer>,
    private val cursors: SyncCursorStore,
    private val auth: AuthRepository,
    private val scope: CoroutineScope,
) {
    private val _status = MutableStateFlow<SyncStatus>(SyncStatus.Idle)
    val status: StateFlow<SyncStatus> = _status.asStateFlow()

    private val mutex = Mutex()

    /** Begin reacting to auth: sync on sign-in, drop cursors on sign-out. Call once at startup. */
    fun start() {
        scope.launch {
            var wasSignedIn = false
            auth.state.collect { state ->
                when (state) {
                    is AuthState.SignedIn -> { wasSignedIn = true; syncNow() }
                    AuthState.SignedOut -> if (wasSignedIn) { wasSignedIn = false; cursors.clear() }
                    AuthState.Unknown -> Unit
                }
            }
        }
    }

    /** Run a full sync now. No-op (returns [SyncOutcome.NotSignedIn]) when signed out. */
    suspend fun syncNow(): SyncOutcome = mutex.withLock {
        if (auth.state.value !is AuthState.SignedIn) return SyncOutcome.NotSignedIn
        _status.value = SyncStatus.Syncing
        val errors = mutableListOf<String>()
        for (syncer in syncers) {
            try {
                val newCursor = syncer.sync(cursors.cursor(syncer.cursorKey))
                if (!newCursor.isNullOrBlank()) cursors.setCursor(syncer.cursorKey, newCursor)
            } catch (e: Exception) {
                errors += "${syncer.cursorKey}: ${e.message ?: e::class.simpleName}"
            }
        }
        return if (errors.isEmpty()) {
            _status.value = SyncStatus.Idle
            SyncOutcome.Success
        } else {
            _status.value = SyncStatus.Failed(errors.map { it.substringBefore(":") })
            SyncOutcome.PartialFailure(errors)
        }
    }
}
