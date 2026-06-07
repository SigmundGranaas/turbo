package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Behavioural guards for the live-sheet primitives. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class LiveWidgetsTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `metric tile is read as one labelled node with value and unit`() {
        composeRule.setContent {
            LiveMetricTile(icon = Icons.Rounded.Speed, label = "Speed", value = "9.1", unit = "km/h", tone = MetricTone.Primary)
        }
        composeRule.onNodeWithContentDescription("Speed: 9.1 km/h").assertIsDisplayed()
    }

    @Test
    fun `wave strip renders without animation in tests`() {
        composeRule.setContent { WaveStrip(color = androidx.compose.ui.graphics.Color.Red, animate = false) }
        composeRule.onNodeWithTag("waveStrip").assertIsDisplayed()
    }

    @Test
    fun `elevation spark renders for a real profile`() {
        composeRule.setContent {
            LiveElevationSpark(
                elevations = listOf(400.0, 520.0, 480.0, 700.0, 812.0),
                progress = 0.62f, label = "Elevation", value = "820 m",
            )
        }
        composeRule.onNodeWithTag("elevationSpark").assertIsDisplayed()
    }
}
