package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class StubConditionsRepository(private val outcome: Outcome<Conditions>) : ConditionsRepository {
    override suspend fun forPoint(point: LatLng): Outcome<Conditions> = outcome
    override suspend fun forecast(point: LatLng): Outcome<com.sigmundgranaas.turbo.expressive.domain.WeatherForecast> =
        Outcome.Failure(UnsupportedOperationException())
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class ConditionsBodyTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val point = LatLng(69.6, 18.9)

    @Test
    fun `weather success renders the temperature tile`() {
        val conditions = Conditions(WeatherNow(-2.0, 4.0, 315.0, 0.2, "cloudy"), null)
        composeRule.setContent {
            ConditionsBody(point, ConditionsViewModel(StubConditionsRepository(Outcome.Success(conditions))))
        }
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("-2°").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("-2°").assertExists()
    }

    @Test
    fun `richer instant fields render the humidity-cloud-UV row`() {
        val conditions = Conditions(
            WeatherNow(5.0, 3.0, 200.0, 0.0, "cloudy", humidityPct = 42.0, cloudCoverPct = 80.0, uvIndex = 3.0),
            null,
        )
        composeRule.setContent {
            ConditionsBody(point, ConditionsViewModel(StubConditionsRepository(Outcome.Success(conditions))))
        }
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Humidity").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("42%").assertExists()
        composeRule.onNodeWithText("UV").assertExists()
    }

    @Test
    fun `failure renders the offline message`() {
        composeRule.setContent {
            ConditionsBody(point, ConditionsViewModel(StubConditionsRepository(Outcome.Failure(RuntimeException()))))
        }
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Conditions unavailable offline.").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Conditions unavailable offline.").assertExists()
    }
}
