package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.sigmundgranaas.turbo.expressive.core.sync.FriendshipDto
import com.sigmundgranaas.turbo.expressive.core.sync.GroupDto
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SharingContentTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun content(
        state: SharingGraphViewModel.UiState,
        onAddFriend: (String) -> Unit = {},
        onAccept: (String) -> Unit = {},
        onCreateGroup: (String) -> Unit = {},
    ) {
        composeRule.setContent {
            MaterialTheme {
                SharingContent(
                    state = state,
                    onBack = {},
                    onMessageShown = {},
                    onAddFriend = onAddFriend,
                    onAccept = onAccept,
                    onDecline = {},
                    onRemoveFriend = {},
                    onCreateGroup = onCreateGroup,
                    onAddGroupMember = { _, _ -> },
                    onRemoveGroupMember = { _, _ -> },
                )
            }
        }
    }

    @Test
    fun `pending request shows accept and accepting fires the callback`() {
        var accepted: String? = null
        content(
            SharingGraphViewModel.UiState(
                loading = false,
                pending = listOf(FriendshipDto(otherUserId = "u-pending", initiatorId = "u-pending", status = "pending")),
            ),
            onAccept = { accepted = it },
        )
        composeRule.onNodeWithTag("accept_u-pending").assertIsDisplayed().performClick()
        assertEquals("u-pending", accepted)
    }

    @Test
    fun `add-friend dialog collects a code and sends it`() {
        var sent: String? = null
        content(SharingGraphViewModel.UiState(loading = false), onAddFriend = { sent = it })
        composeRule.onNodeWithTag("sharingAction").performClick()
        composeRule.onNodeWithTag("codeEntryField").performTextInput("turbo-AB12")
        composeRule.onNodeWithTag("codeEntryConfirm").performClick()
        assertEquals("turbo-AB12", sent)
    }

    @Test
    fun `groups tab lists groups and create fires the callback`() {
        var created: String? = null
        content(
            SharingGraphViewModel.UiState(loading = false, groups = listOf(GroupDto(id = "g-1", name = "Tindetur"))),
            onCreateGroup = { created = it },
        )
        composeRule.onNodeWithTag("sharingTab_GROUPS").performClick()
        composeRule.onNodeWithTag("group_g-1").assertIsDisplayed()
        composeRule.onNodeWithTag("sharingAction").performClick()
        composeRule.onNodeWithTag("codeEntryField").performTextInput("Skitur")
        composeRule.onNodeWithTag("codeEntryConfirm").performClick()
        assertEquals("Skitur", created)
    }

    @Test
    fun `empty friends shows the empty state`() {
        content(SharingGraphViewModel.UiState(loading = false))
        composeRule.onNodeWithText("No friends yet").assertIsDisplayed()
    }

    @Test
    fun `a state message is surfaced as a snackbar`() {
        content(SharingGraphViewModel.UiState(loading = false, message = "No user found for that code"))
        composeRule.onNodeWithText("No user found for that code").assertIsDisplayed()
    }
}
