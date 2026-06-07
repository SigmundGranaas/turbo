package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.ui.test.assertIsDisplayed
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
 * Validates the home-map recording surface: the controls render the live journey
 * stats and route pause/stop intent back to the caller. Together with
 * [ActiveJourneyTest] (session → journey resolution) this proves recording works
 * as a mode of the map without a standalone screen.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class RecordingControlsTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun recording(paused: Boolean = false) = ActiveJourney(
        mode = JourneyMode.Recording,
        distanceM = 1234.0,
        elapsedSec = 125,
        paused = paused,
    )

    @Test
    fun `recording shows live stats and both controls`() {
        composeRule.setContent {
            RecordingControls(journey = recording(), metric = true, onPause = {}, onStop = {})
        }
        composeRule.onNodeWithTag("recStatus").assertIsDisplayed()
        composeRule.onNodeWithText("Recording").assertExists()
        composeRule.onNodeWithTag("recDistance").assertIsDisplayed()
        composeRule.onNodeWithText("02:05").assertExists() // 125s elapsed
        composeRule.onNodeWithTag("recPause").assertIsDisplayed()
        composeRule.onNodeWithTag("recStop").assertIsDisplayed()
    }

    @Test
    fun `stop button reports stop intent`() {
        var stopped = false
        composeRule.setContent {
            RecordingControls(journey = recording(), metric = true, onPause = {}, onStop = { stopped = true })
        }
        composeRule.onNodeWithTag("recStop").performClick()
        assert(stopped)
    }

    @Test
    fun `pause button reports pause intent`() {
        var paused = false
        composeRule.setContent {
            RecordingControls(journey = recording(), metric = true, onPause = { paused = true }, onStop = {})
        }
        composeRule.onNodeWithTag("recPause").performClick()
        assert(paused)
    }

    @Test
    fun `paused journey reads as paused`() {
        composeRule.setContent {
            RecordingControls(journey = recording(paused = true), metric = true, onPause = {}, onStop = {})
        }
        composeRule.onNodeWithText("Paused").assertExists()
    }
}
