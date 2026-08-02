package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The controls that moved off the main Settings list.
 *
 * Two things need pinning after a move like this: that everything
 * actually arrived, and that it is genuinely gone from where it was.
 * A move that only did the first half leaves the screen exactly as
 * cluttered as before, with a second copy elsewhere.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class AdvancedSettingsScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun show() {
        composeRule.setContent {
            AdvancedSettingsScreen(
                onBack = {},
                viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()),
            )
        }
    }

    @Test
    fun `the gesture tunables live here now`() {
        show()
        composeRule.onNodeWithTag("gestureLongPress").performScrollTo().assertExists()
        composeRule.onNodeWithTag("gestureRotation").assertExists()
        composeRule.onNodeWithTag("gestureMoveGuard").assertExists()
        composeRule.onNodeWithTag("gestureFlick").assertExists()
    }

    @Test
    fun `the experimental layers live here now`() {
        show()
        composeRule.onNodeWithTag("experimentalTrails").performScrollTo().assertExists()
        composeRule.onNodeWithTag("experimentalClouds").assertExists()
    }

    @Test
    fun `the routing engine controls live here now`() {
        show()
        composeRule.onNodeWithTag("routeEngine_Auto").performScrollTo().assertExists()
        composeRule.onNodeWithTag("packSourceField").assertExists()
        composeRule.onNodeWithTag("routeShadowCompare").assertExists()
    }

    /** Sections are titled — the whole reason for the split. */
    @Test
    fun `each group carries a heading`() {
        show()
        composeRule.onNodeWithText("Gestures").assertExists()
        composeRule.onNodeWithText("Experimental").assertExists()
        composeRule.onNodeWithText("Routing engine").assertExists()
    }

    @Test
    fun `the shadow-compare toggle still works from here`() {
        show()
        composeRule.onNodeWithTag("routeShadowCompare").performScrollTo().performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithTag("routeShadowCompare").assertExists()
    }
}
