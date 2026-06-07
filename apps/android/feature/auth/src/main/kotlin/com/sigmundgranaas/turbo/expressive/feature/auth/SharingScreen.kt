package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Group
import androidx.compose.material.icons.rounded.GroupAdd
import androidx.compose.material.icons.rounded.Inbox
import androidx.compose.material.icons.rounded.PersonAdd
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.core.sync.FriendshipDto
import com.sigmundgranaas.turbo.expressive.core.sync.GroupDto
import com.sigmundgranaas.turbo.expressive.core.sync.ResourceEnvelopeDto
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth

private enum class SharingTab { FRIENDS, GROUPS, SHARED }

@Composable
fun SharingScreen(
    onBack: () -> Unit,
    viewModel: SharingGraphViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    SharingContent(
        state = state,
        onBack = onBack,
        onMessageShown = viewModel::clearMessage,
        onAddFriend = viewModel::addFriend,
        onAccept = viewModel::accept,
        onDecline = viewModel::decline,
        onRemoveFriend = viewModel::removeFriend,
        onCreateGroup = viewModel::createGroup,
        onAddGroupMember = viewModel::addGroupMember,
        onRemoveGroupMember = viewModel::removeGroupMember,
    )
}

/** Stateless body — host-free so it can be exercised headlessly. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SharingContent(
    state: SharingGraphViewModel.UiState,
    onBack: () -> Unit,
    onMessageShown: () -> Unit,
    onAddFriend: (String) -> Unit,
    onAccept: (String) -> Unit,
    onDecline: (String) -> Unit,
    onRemoveFriend: (String) -> Unit,
    onCreateGroup: (String) -> Unit,
    onAddGroupMember: (String, String) -> Unit,
    onRemoveGroupMember: (String, String) -> Unit,
) {
    val snackbar = remember { SnackbarHostState() }
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var addFriend by remember { mutableStateOf(false) }
    var createGroup by remember { mutableStateOf(false) }
    var openGroup by remember { mutableStateOf<GroupDto?>(null) }

    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            onMessageShown()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.sharing_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.auth_back))
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .responsiveContentWidth(560.dp),
        ) {
            SingleChoiceSegmentedButtonRow(
                Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 8.dp),
            ) {
                SharingTab.entries.forEachIndexed { i, t ->
                    SegmentedButton(
                        selected = tab == i,
                        onClick = { tab = i },
                        shape = SegmentedButtonDefaults.itemShape(i, SharingTab.entries.size),
                        modifier = Modifier.testTag("sharingTab_${t.name}"),
                    ) {
                        Text(
                            stringResource(
                                when (t) {
                                    SharingTab.FRIENDS -> R.string.sharing_tab_friends
                                    SharingTab.GROUPS -> R.string.sharing_tab_groups
                                    SharingTab.SHARED -> R.string.sharing_tab_shared
                                },
                            ),
                        )
                    }
                }
            }

            when (SharingTab.entries[tab]) {
                SharingTab.FRIENDS -> FriendsTab(
                    state = state,
                    onAdd = { addFriend = true },
                    onAccept = onAccept,
                    onDecline = onDecline,
                    onRemove = onRemoveFriend,
                )
                SharingTab.GROUPS -> GroupsTab(
                    groups = state.groups,
                    loading = state.loading,
                    onCreate = { createGroup = true },
                    onOpen = { openGroup = it },
                )
                SharingTab.SHARED -> SharedTab(shared = state.shared, loading = state.loading)
            }
        }
    }

    if (addFriend) {
        CodeEntryDialog(
            title = stringResource(R.string.sharing_add_friend),
            label = stringResource(R.string.sharing_friend_code_label),
            confirm = stringResource(R.string.sharing_send_request),
            busy = state.busy,
            onConfirm = { onAddFriend(it); addFriend = false },
            onDismiss = { addFriend = false },
        )
    }
    if (createGroup) {
        CodeEntryDialog(
            title = stringResource(R.string.sharing_create_group),
            label = stringResource(R.string.sharing_group_name_label),
            confirm = stringResource(R.string.sharing_create),
            busy = state.busy,
            onConfirm = { onCreateGroup(it); createGroup = false },
            onDismiss = { createGroup = false },
        )
    }
    openGroup?.let { group ->
        // Re-resolve the group from latest state so member changes reflect live.
        val live = state.groups.firstOrNull { it.id == group.id } ?: group
        GroupDetailDialog(
            group = live,
            busy = state.busy,
            onAddMember = { onAddGroupMember(live.id, it) },
            onRemoveMember = { onRemoveGroupMember(live.id, it) },
            onDismiss = { openGroup = null },
        )
    }
}

@Composable
private fun FriendsTab(
    state: SharingGraphViewModel.UiState,
    onAdd: () -> Unit,
    onAccept: (String) -> Unit,
    onDecline: (String) -> Unit,
    onRemove: (String) -> Unit,
) {
    if (!state.loading && state.accepted.isEmpty() && state.pending.isEmpty()) {
        TabScaffold(onAction = onAdd, actionIcon = { Icon(Icons.Rounded.PersonAdd, null) }, actionLabel = stringResource(R.string.sharing_add_friend)) {
            EmptyState(
                icon = Icons.Rounded.PersonAdd,
                title = stringResource(R.string.sharing_no_friends),
                body = stringResource(R.string.sharing_no_friends_hint),
            )
        }
        return
    }
    TabScaffold(onAction = onAdd, actionIcon = { Icon(Icons.Rounded.PersonAdd, null) }, actionLabel = stringResource(R.string.sharing_add_friend)) {
        LazyColumn(
            Modifier.fillMaxSize().testTag("friendsList"),
            contentPadding = PaddingValues(20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (state.pending.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.sharing_pending)) }
                items(state.pending, key = { "p_${it.otherUserId}" }) { f ->
                    PendingRow(f, onAccept = { onAccept(f.otherUserId) }, onDecline = { onDecline(f.otherUserId) })
                }
            }
            if (state.accepted.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.sharing_friends)) }
                items(state.accepted, key = { "f_${it.otherUserId}" }) { f ->
                    FriendRow(f.otherUserId, onRemove = { onRemove(f.otherUserId) })
                }
            }
        }
    }
}

@Composable
private fun GroupsTab(groups: List<GroupDto>, loading: Boolean, onCreate: () -> Unit, onOpen: (GroupDto) -> Unit) {
    if (!loading && groups.isEmpty()) {
        TabScaffold(onAction = onCreate, actionIcon = { Icon(Icons.Rounded.GroupAdd, null) }, actionLabel = stringResource(R.string.sharing_create_group)) {
            EmptyState(
                icon = Icons.Rounded.Group,
                title = stringResource(R.string.sharing_no_groups),
                body = stringResource(R.string.sharing_no_groups_hint),
            )
        }
        return
    }
    TabScaffold(onAction = onCreate, actionIcon = { Icon(Icons.Rounded.GroupAdd, null) }, actionLabel = stringResource(R.string.sharing_create_group)) {
        LazyColumn(
            Modifier.fillMaxSize().testTag("groupsList"),
            contentPadding = PaddingValues(20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(groups, key = { it.id }) { g ->
                GroupRow(g, onClick = { onOpen(g) })
            }
        }
    }
}

@Composable
private fun SharedTab(shared: List<ResourceEnvelopeDto>, loading: Boolean) {
    if (!loading && shared.isEmpty()) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            EmptyState(
                icon = Icons.Rounded.Inbox,
                title = stringResource(R.string.sharing_nothing_shared),
                body = stringResource(R.string.sharing_nothing_shared_hint),
            )
        }
        return
    }
    LazyColumn(
        Modifier.fillMaxSize().testTag("sharedList"),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        items(shared, key = { "${it.type}_${it.id}" }) { r -> SharedRow(r) }
    }
}

// ─────────────────────────── rows + bits ───────────────────────────

@Composable
private fun TabScaffold(
    onAction: () -> Unit,
    actionIcon: @Composable () -> Unit,
    actionLabel: String,
    content: @Composable () -> Unit,
) {
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.weight(1f)) { content() }
        FilledTonalButton(
            onClick = onAction,
            modifier = Modifier.fillMaxWidth().padding(20.dp).height(52.dp).testTag("sharingAction"),
        ) {
            actionIcon()
            Spacer(Modifier.width(8.dp))
            Text(actionLabel)
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700, letterSpacing = 1.sp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 6.dp, bottom = 2.dp),
    )
}

@Composable
private fun Monogram(seed: String, size: Int = 44) {
    val cs = MaterialTheme.colorScheme
    val palette = listOf(cs.primaryContainer, cs.secondaryContainer, cs.tertiaryContainer)
    val onPalette = listOf(cs.onPrimaryContainer, cs.onSecondaryContainer, cs.onTertiaryContainer)
    val idx = (seed.hashCode().rem(palette.size).let { (it + palette.size) % palette.size })
    Box(
        Modifier.size(size.dp).background(palette[idx], CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            seed.firstOrNull { it.isLetterOrDigit() }?.uppercaseChar()?.toString() ?: "?",
            style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W800),
            color = onPalette[idx],
        )
    }
}

@Composable
private fun PendingRow(f: FriendshipDto, onAccept: () -> Unit, onDecline: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().background(cs.primaryContainer, MaterialTheme.shapes.large).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Monogram(f.otherUserId)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.sharing_friend_request), style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700), color = cs.onPrimaryContainer)
            Text(shortId(f.otherUserId), style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer.copy(alpha = 0.8f))
        }
        IconButton(onClick = onDecline, modifier = Modifier.testTag("decline_${f.otherUserId}")) {
            Icon(Icons.Rounded.Close, stringResource(R.string.sharing_decline), tint = cs.onPrimaryContainer)
        }
        IconButton(onClick = onAccept, modifier = Modifier.testTag("accept_${f.otherUserId}")) {
            Icon(Icons.Rounded.Check, stringResource(R.string.sharing_accept), tint = cs.onPrimaryContainer)
        }
    }
}

@Composable
private fun FriendRow(userId: String, onRemove: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().background(cs.surfaceContainerLow, MaterialTheme.shapes.large).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Monogram(userId)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.sharing_friend), style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W600), color = cs.onSurface)
            Text(shortId(userId), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        TextButton(onClick = onRemove, modifier = Modifier.testTag("removeFriend_$userId")) {
            Text(stringResource(R.string.sharing_remove), color = cs.error)
        }
    }
}

@Composable
private fun GroupRow(g: GroupDto, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().background(cs.surfaceContainerLow, MaterialTheme.shapes.large)
            .clickable(onClick = onClick).padding(14.dp).testTag("group_${g.id}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Monogram(g.name.ifBlank { g.id })
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(g.name.ifBlank { stringResource(R.string.sharing_group) }, style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700), color = cs.onSurface)
            Text(
                stringResource(R.string.sharing_member_count, g.members.size),
                style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun SharedRow(r: ResourceEnvelopeDto) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().background(cs.surfaceContainerLow, MaterialTheme.shapes.large).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Monogram(r.type)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(r.type.replaceFirstChar { it.uppercase() }, style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W600), color = cs.onSurface)
            Text(
                r.myRole?.let { stringResource(R.string.sharing_role_fmt, it) } ?: shortId(r.id),
                style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GroupDetailDialog(
    group: GroupDto,
    busy: Boolean,
    onAddMember: (String) -> Unit,
    onRemoveMember: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var code by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.sharing_done)) } },
        title = { Text(group.name.ifBlank { stringResource(R.string.sharing_group) }) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                group.members.forEach { m ->
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Monogram(m.userId, size = 36)
                        Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            Text(shortId(m.userId), style = MaterialTheme.typography.bodyMedium)
                            Text(m.role, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        IconButton(onClick = { onRemoveMember(m.userId) }, modifier = Modifier.testTag("removeMember_${m.userId}")) {
                            Icon(Icons.Rounded.Close, stringResource(R.string.sharing_remove))
                        }
                    }
                }
                Spacer(Modifier.height(4.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it },
                    label = { Text(stringResource(R.string.sharing_friend_code_label)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().testTag("groupMemberCode"),
                )
                FilledTonalButton(
                    onClick = { onAddMember(code); code = "" },
                    enabled = !busy && code.isNotBlank(),
                    modifier = Modifier.fillMaxWidth().testTag("addMember"),
                ) { Text(stringResource(R.string.sharing_add_member)) }
            }
        },
    )
}

@Composable
private fun CodeEntryDialog(
    title: String,
    label: String,
    confirm: String,
    busy: Boolean,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var value by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = value,
                onValueChange = { value = it },
                label = { Text(label) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().testTag("codeEntryField"),
            )
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(value) },
                enabled = !busy && value.isNotBlank(),
                modifier = Modifier.testTag("codeEntryConfirm"),
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp) else Text(confirm)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.sharing_cancel)) } },
    )
}

/** First 8 chars of an opaque id, for a friendly-but-honest label. */
private fun shortId(id: String): String = if (id.length <= 10) id else id.take(8) + "…"
