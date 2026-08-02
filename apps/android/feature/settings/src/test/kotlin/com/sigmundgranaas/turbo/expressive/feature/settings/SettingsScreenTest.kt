package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.sigmundgranaas.turbo.expressive.core.auth.Account
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode


@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])


class SettingsScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `the account header shows the real signed-in identity`() {
        composeRule.setContent {
            SettingsScreen(
                onBack = {},
                viewModel = SettingsViewModel(
                    FakeSettingsRepository(),
                    FakeAuthRepository(AuthState.SignedIn(Account("a-1", "hiker@x.no"))),
                ),
            )
        }
        // The user's actual email — not a hardcoded identity block.
        composeRule.onNodeWithText("hiker@x.no").assertExists()
    }

    @Test
    fun `signed out shows a sign-in prompt, not a fake identity`() {
        composeRule.setContent {
            SettingsScreen(
                onBack = {},
                viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository(AuthState.SignedOut)),
            )
        }
        composeRule.onNodeWithText("Sign in").assertExists()
        composeRule.onAllNodesWithText("Sigmund G.").assertCountEquals(0)
    }

    @Test
    fun `toggling the units switch flips metric to imperial`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()))
        }
        // Default is metric.
        composeRule.onNodeWithText("Metric · km, m").assertExists()

        composeRule.onNodeWithTag("unitsSwitch").performScrollTo().performClick()

        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Imperial · mi, ft").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Imperial · mi, ft").assertExists()
    }

    @Test
    fun `selecting the Dark theme chip updates the appearance subtitle`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()))
        }
        composeRule.onNodeWithText("Follow system").assertExists() // System default

        composeRule.onNodeWithTag("theme_Dark").performScrollTo().performClick()

        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Dark theme").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Dark theme").assertExists()
    }

    /**
     * The advanced controls are gone from here, not duplicated.
     *
     * This is the half of the move that is easy to skip: if they still
     * render on the main screen too, the list is exactly as long as it
     * was and the split bought nothing.
     */
    @Test
    fun `advanced controls are behind the Advanced door, not on this screen`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()))
        }
        composeRule.onNodeWithTag("openAdvanced").performScrollTo().assertExists()
        composeRule.onNodeWithTag("gestureLongPress").assertDoesNotExist()
        composeRule.onNodeWithTag("experimentalTrails").assertDoesNotExist()
        composeRule.onNodeWithTag("packSourceField").assertDoesNotExist()
        composeRule.onNodeWithTag("routeEngine_Auto").assertDoesNotExist()
    }

    /** Tapping it navigates rather than expanding in place. */
    @Test
    fun `the Advanced row opens the advanced screen`() {
        var opened = false
        composeRule.setContent {
            SettingsScreen(
                onBack = {},
                onOpenAdvanced = { opened = true },
                viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()),
            )
        }
        composeRule.onNodeWithTag("openAdvanced").performScrollTo().performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertTrue("Advanced must navigate", opened)
    }

    /**
     * Building packs on the phone stays in front of the user. It is not a
     * measurement knob — it changes what tapping Download does, by minutes.
     */
    @Test
    fun `building packs on device stays on the main screen`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()))
        }
        composeRule.onNodeWithTag("buildPacksOnDevice").performScrollTo().assertExists()
    }

    /** Every group is named — the fix for "one long undifferentiated list". */
    @Test
    fun `the sections carry headings`() {
        composeRule.setContent {
            SettingsScreen(onBack = {}, viewModel = SettingsViewModel(FakeSettingsRepository(), FakeAuthRepository()))
        }
        composeRule.onNodeWithText("Appearance").assertExists()
        composeRule.onNodeWithText("Map").assertExists()
        composeRule.onNodeWithText("General").assertExists()
        composeRule.onNodeWithText("Offline maps").performScrollTo().assertExists()
    }
}
