package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.sigmundgranaas.turbo.expressive.domain.AtmosphericPoint
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherSummary
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class WeatherForecastContentTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun pt(time: String, temp: Double) = AtmosphericPoint(
        timeIso = time, temperatureC = temp, windSpeedMs = 3.0, windFromDeg = 200.0,
        humidityPct = null, cloudCoverPct = null, uvIndex = null,
        precipitation1hMm = 0.0, symbol1h = "cloudy",
    )

    private val forecast = run {
        // 2026-06-05 is a Saturday, 2026-06-06 a Sunday.
        val points = listOf(
            pt("2026-06-05T09:00:00Z", 7.0),   // Sat (unique 09:00)
            pt("2026-06-05T12:00:00Z", 11.0),  // Sat
            pt("2026-06-06T15:00:00Z", 13.0),  // Sun (unique 15:00)
        )
        WeatherForecast(points, WeatherSummary.dailySummaries(points))
    }

    @Test
    fun `renders a day chip per day and the first day's hourly rows`() {
        composeRule.setContent { WeatherForecastContent(forecast) }

        // First day (2 points) is selected by default → its 09:00 + 12:00 hours show.
        composeRule.onNodeWithText("09:00").assertIsDisplayed()
        composeRule.onNodeWithText("12:00").assertIsDisplayed()
        // Both day chips render their min/max summary (Sat 11/7, Sun 13/13).
        composeRule.onNodeWithText("11° / 7°").assertExists()
        composeRule.onNodeWithText("13° / 13°").assertExists()
    }
}
