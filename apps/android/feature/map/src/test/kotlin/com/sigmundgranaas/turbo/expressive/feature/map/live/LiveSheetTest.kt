package com.sigmundgranaas.turbo.expressive.feature.map.live

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.core.data.LiveMode
import com.sigmundgranaas.turbo.expressive.core.data.LiveStats
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
class LiveSheetTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val recStats = LiveStats(
        mode = LiveMode.Recording, distanceM = 6_200.0, elapsedSec = 2_892,
        speedMps = 2.5, maxSpeedMps = 3.9, ascentM = 412.0, descentM = 138.0, altitudeM = 812.0, kcal = 486,
    )

    private val followStats = LiveStats(
        mode = LiveMode.Following, distanceM = 6_200.0, distanceRemainingM = 4_100.0, routeDistanceM = 10_300.0,
        speedMps = 2.5, ascentM = 460.0, descentM = 180.0, ascentRemainingM = 175.0, etaSeconds = 2_520, fraction = 0.62, kcal = 486,
    )

    @Test
    fun `recording full detent shows the bento tiles and Finish`() {
        composeRule.setContent {
            LiveSheet(
                stats = recStats, metric = true, title = "48:12",
                detent = LiveDetent.Full, onDetentChange = {}, onTogglePause = {}, onStop = {},
                elevations = listOf(400.0, 520.0, 700.0, 812.0),
            )
        }
        composeRule.onNodeWithTag("liveStatus").assertIsDisplayed() // REC
        composeRule.onNodeWithContentDescription("Speed: 9.0 km/h").assertIsDisplayed()
        // The second bento row + elevation live below the fold in the scroll area, but exist…
        composeRule.onNodeWithContentDescription("Ascent: 412 m").assertExists()
        composeRule.onNodeWithTag("elevationSpark").assertExists()
        // …while the action bar is pinned and always visible.
        composeRule.onNodeWithTag("liveFinish").assertIsDisplayed()
    }

    @Test
    fun `recording peek detent hides tiles, shows compact controls`() {
        composeRule.setContent {
            LiveSheet(
                stats = recStats, metric = true, title = "48:12",
                detent = LiveDetent.Peek, onDetentChange = {}, onTogglePause = {}, onStop = {},
            )
        }
        composeRule.onNodeWithTag("livePause").assertIsDisplayed()
        composeRule.onNodeWithContentDescription("Speed: 9.0 km/h").assertDoesNotExist()
    }

    @Test
    fun `tapping pause fires the toggle`() {
        var toggled = false
        composeRule.setContent {
            LiveSheet(
                stats = recStats, metric = true, title = "48:12",
                detent = LiveDetent.Full, onDetentChange = {}, onTogglePause = { toggled = true }, onStop = {},
            )
        }
        composeRule.onNodeWithTag("livePause").performClick()
        assertEquals(true, toggled)
    }

    @Test
    fun `tapping the handle expands one detent`() {
        var detent = LiveDetent.Peek
        composeRule.setContent {
            LiveSheet(
                stats = recStats, metric = true, title = "48:12",
                detent = detent, onDetentChange = { detent = it }, onTogglePause = {}, onStop = {},
            )
        }
        composeRule.onNodeWithTag("liveGrab").performClick()
        assertEquals(LiveDetent.Half, detent)
    }

    @Test
    fun `following full detent shows progress, route tiles and Stop following`() {
        composeRule.setContent {
            LiveSheet(
                stats = followStats, metric = true, title = "Skåla Loop",
                detent = LiveDetent.Full, onDetentChange = {}, onTogglePause = {}, onStop = {},
                nextWaypoint = "Next waypoint · 4.1 km" to "Skåla Loop",
            )
        }
        composeRule.onNodeWithTag("liveStatus").assertIsDisplayed() // On route
        composeRule.onNodeWithTag("liveProgressFill").assertIsDisplayed()
        // Accumulated distance + gain + loss are always visible in the follow glance (US-1).
        composeRule.onNodeWithTag("liveCovered").assertIsDisplayed()
        composeRule.onNodeWithContentDescription("To climb: 175 m").assertIsDisplayed()
        composeRule.onNodeWithTag("liveStop").assertIsDisplayed()
    }

    @Test
    fun `following mini detent collapses to a one-liner with the stop control`() {
        composeRule.setContent {
            LiveSheet(
                stats = followStats, metric = true, title = "Skåla Loop",
                detent = LiveDetent.Mini, onDetentChange = {}, onTogglePause = {}, onStop = {},
            )
        }
        // The compact bar keeps the title + the stop control, but drops the tall hero
        // (hero number) and the bento entirely.
        composeRule.onNodeWithTag("liveTitle").assertIsDisplayed()
        composeRule.onNodeWithTag("liveStop").assertIsDisplayed()
        composeRule.onNodeWithTag("liveHeroNumber").assertDoesNotExist()
        composeRule.onNodeWithContentDescription("To climb: 175 m").assertDoesNotExist()
    }

    @Test
    fun `recording mini detent shows compact controls and no hero`() {
        composeRule.setContent {
            LiveSheet(
                stats = recStats, metric = true, title = "48:12",
                detent = LiveDetent.Mini, onDetentChange = {}, onTogglePause = {}, onStop = {},
            )
        }
        composeRule.onNodeWithTag("livePause").assertIsDisplayed()
        composeRule.onNodeWithTag("liveHeroNumber").assertDoesNotExist()
    }

    @Test
    fun `following hero shows distance remaining as the hero number`() {
        composeRule.setContent {
            LiveSheet(
                stats = followStats, metric = true, title = "Skåla Loop",
                detent = LiveDetent.Half, onDetentChange = {}, onTogglePause = {}, onStop = {},
            )
        }
        composeRule.onNodeWithTag("liveHeroNumber").assertIsDisplayed()
        composeRule.onNodeWithText("4.1").assertIsDisplayed()
    }
}
