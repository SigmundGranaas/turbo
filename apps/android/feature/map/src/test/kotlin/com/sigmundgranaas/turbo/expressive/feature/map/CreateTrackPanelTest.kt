package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.CreateTrackPanel
import com.sigmundgranaas.turbo.expressive.feature.map.route.TrackDetent
import com.sigmundgranaas.turbo.expressive.feature.map.route.TrackMode

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Validates the unified Create-track panel: the segmented mode toggle, the hero
 * stat, the route-only zones, the round-trip toggle, detent-driven detail, and that
 * Save/Follow gate on having a real track.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class CreateTrackPanelTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun panel(
        mode: TrackMode,
        canSave: Boolean = true,
        roundTrip: Boolean = false,
        detent: TrackDetent = TrackDetent.Default,
        onMode: (TrackMode) -> Unit = {},
        onSave: () -> Unit = {},
        onFollow: () -> Unit = {},
        onToggleRoundTrip: () -> Unit = {},
    ): @androidx.compose.runtime.Composable () -> Unit = {
        CreateTrackPanel(
            mode = mode,
            onMode = onMode,
            distanceText = "8.4",
            unit = "km",
            metaText = "2 h 40 min",
            surfaces = mapOf("trail" to 780.0, "road" to 140.0, "off-trail" to 80.0),
            roundTrip = roundTrip,
            onToggleRoundTrip = onToggleRoundTrip,
            detent = detent,
            canUndo = true,
            canSave = canSave,
            onUndo = {},
            onClear = {},
            onSave = onSave,
            onFollow = onFollow,
        )
    }

    @Test
    fun `route mode shows the hero stat, the three modes, and the round-trip toggle`() {
        composeRule.setContent { panel(TrackMode.Route)() }
        composeRule.onNodeWithTag("trackDistance").assertIsDisplayed()
        composeRule.onNodeWithText("8.4").assertExists()
        composeRule.onNodeWithTag("trackMode_Route").assertIsDisplayed()
        composeRule.onNodeWithTag("trackMode_Line").assertIsDisplayed()
        composeRule.onNodeWithTag("trackMode_Draw").assertIsDisplayed()
        composeRule.onNodeWithTag("roundTripRow").assertIsDisplayed()
    }

    @Test
    fun `route mode exposes no route-style selector (Phase 4 removed it)`() {
        composeRule.setContent { panel(TrackMode.Route)() }
        composeRule.onNodeWithTag("routeStyleRow").assertDoesNotExist()
    }

    @Test
    fun `line mode hides the route-only round-trip toggle`() {
        composeRule.setContent { panel(TrackMode.Line)() }
        composeRule.onNodeWithTag("trackMode_Line").assertIsDisplayed()
        composeRule.onNodeWithTag("roundTripRow").assertDoesNotExist()
    }

    @Test
    fun `collapsing the detent folds away the route detail, keeps the primary controls`() {
        composeRule.setContent { panel(TrackMode.Route, detent = TrackDetent.Collapsed)() }
        // Primary controls always present.
        composeRule.onNodeWithTag("trackMode_Route").assertIsDisplayed()
        composeRule.onNodeWithTag("trackSave").assertExists()
        // Route detail (round-trip toggle) folds away at the collapsed stop.
        composeRule.onNodeWithTag("roundTripRow").assertDoesNotExist()
    }

    @Test
    fun `tapping the round-trip toggle reports it`() {
        var toggled = false
        composeRule.setContent { panel(TrackMode.Route, onToggleRoundTrip = { toggled = true })() }
        composeRule.onNodeWithTag("roundTripRow").performClick()
        assert(toggled)
    }

    @Test
    fun `tapping a different mode reports it`() {
        var picked: TrackMode? = null
        composeRule.setContent { panel(TrackMode.Route, onMode = { picked = it })() }
        composeRule.onNodeWithTag("trackMode_Draw").performClick()
        assert(picked == TrackMode.Draw)
    }

    @Test
    fun `save and follow fire when there is a track`() {
        var saved = false
        var followed = false
        composeRule.setContent { panel(TrackMode.Route, canSave = true, onSave = { saved = true }, onFollow = { followed = true })() }
        composeRule.onNodeWithTag("trackSave").assertIsEnabled().performClick()
        composeRule.onNodeWithTag("trackFollow").assertIsEnabled().performClick()
        assert(saved)
        assert(followed)
    }

    @Test
    fun `save and follow are disabled without a track`() {
        composeRule.setContent { panel(TrackMode.Line, canSave = false)() }
        composeRule.onNodeWithTag("trackSave").assertIsNotEnabled()
        composeRule.onNodeWithTag("trackFollow").assertIsNotEnabled()
    }
}
