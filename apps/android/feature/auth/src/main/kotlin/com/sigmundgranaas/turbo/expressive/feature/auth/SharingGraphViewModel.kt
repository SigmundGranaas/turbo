package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.sync.FriendshipDto
import com.sigmundgranaas.turbo.expressive.core.sync.GroupDto
import com.sigmundgranaas.turbo.expressive.core.sync.ResourceEnvelopeDto
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Friends + groups + shared-with-you, backed by the Sharing service. */
@HiltViewModel
class SharingGraphViewModel @Inject constructor(
    private val sharing: SharingRepository,
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        val accepted: List<FriendshipDto> = emptyList(),
        val pending: List<FriendshipDto> = emptyList(),
        val groups: List<GroupDto> = emptyList(),
        val shared: List<ResourceEnvelopeDto> = emptyList(),
        /** A transient user-facing message (add-friend result, errors). */
        val message: String? = null,
        val busy: Boolean = false,
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init { refresh() }

    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(loading = true) }
            val friends = sharing.friendships().getOrNull().orEmpty()
            val groups = sharing.groups().getOrNull().orEmpty()
            val shared = sharing.sharedResources(null).getOrNull()?.items.orEmpty().filterNot { it.deleted }
            _state.update {
                it.copy(
                    loading = false,
                    accepted = friends.filter { f -> f.status.equals("accepted", ignoreCase = true) },
                    pending = friends.filter { f -> f.status.equals("pending", ignoreCase = true) },
                    groups = groups,
                    shared = shared,
                )
            }
        }
    }

    /** Look up a friend code and send a friend request. */
    fun addFriend(code: String) {
        val trimmed = code.trim()
        if (trimmed.isBlank()) return
        viewModelScope.launch {
            _state.update { it.copy(busy = true, message = null) }
            val msg = when (val lookup = sharing.lookupUser(trimmed)) {
                is Outcome.Success ->
                    when (sharing.requestFriendship(lookup.value)) {
                        is Outcome.Success -> "Friend request sent"
                        is Outcome.Failure -> "Couldn't send request"
                    }
                is Outcome.Failure -> "No user found for that code"
            }
            _state.update { it.copy(busy = false, message = msg) }
            refresh()
        }
    }

    fun accept(otherUserId: String) = act { sharing.acceptFriendship(otherUserId) }
    fun decline(otherUserId: String) = act { sharing.removeFriendship(otherUserId) }
    fun removeFriend(otherUserId: String) = act { sharing.removeFriendship(otherUserId) }

    fun createGroup(name: String) {
        if (name.isBlank()) return
        act { sharing.createGroup(name.trim()) }
    }

    fun addGroupMember(groupId: String, code: String) {
        val trimmed = code.trim()
        if (trimmed.isBlank()) return
        viewModelScope.launch {
            _state.update { it.copy(busy = true, message = null) }
            val msg = when (val lookup = sharing.lookupUser(trimmed)) {
                is Outcome.Success ->
                    when (sharing.addGroupMember(groupId, lookup.value)) {
                        is Outcome.Success -> "Member added"
                        is Outcome.Failure -> "Couldn't add member"
                    }
                is Outcome.Failure -> "No user found for that code"
            }
            _state.update { it.copy(busy = false, message = msg) }
            refresh()
        }
    }

    fun removeGroupMember(groupId: String, userId: String) = act { sharing.removeGroupMember(groupId, userId) }

    fun clearMessage() = _state.update { it.copy(message = null) }

    private fun act(block: suspend () -> Outcome<*>) {
        viewModelScope.launch {
            _state.update { it.copy(busy = true) }
            block()
            _state.update { it.copy(busy = false) }
            refresh()
        }
    }
}
