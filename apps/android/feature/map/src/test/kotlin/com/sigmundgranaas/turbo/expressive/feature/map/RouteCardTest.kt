package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
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
class RouteCardTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val plan = RoutePlan(
        distanceM = 8537.0, durationS = 3600.0, ascentM = 234.0, onTrailPct = 78.0,
        surfaces = emptyMap(),
        geometry = listOf(LatLng(69.0, 18.0), LatLng(69.01, 18.01)),
    )

    @Test
    fun `done state shows stats and the action buttons`() {
        composeRule.setContent {
            RouteCard(
                state = RouteUiState.Done(plan),
                preset = RoutePreset.Balanced,
                userLocation = null,
                onSelectPreset = {}, onFollow = {}, onSave = {}, onClear = {},
            )
        }
        composeRule.onNodeWithText("8.5 km").assertExists()
        composeRule.onNodeWithText("Follow").assertExists()
        composeRule.onNodeWithText("Clear").assertExists()
    }

    @Test
    fun `tapping a preset chip reports the selection`() {
        var picked: RoutePreset? = null
        composeRule.setContent {
            RouteCard(
                state = RouteUiState.Done(plan),
                preset = RoutePreset.Balanced,
                userLocation = null,
                onSelectPreset = { picked = it }, onFollow = {}, onSave = {}, onClear = {},
            )
        }
        composeRule.onNodeWithText("Avoid roads").performClick()
        assertEquals(RoutePreset.AvoidRoads, picked)
    }

    @Test
    fun `follow button invokes the follow callback`() {
        var followed = false
        composeRule.setContent {
            RouteCard(
                state = RouteUiState.Done(plan),
                preset = RoutePreset.Balanced,
                userLocation = null,
                onSelectPreset = {}, onFollow = { followed = true }, onSave = {}, onClear = {},
            )
        }
        composeRule.onNodeWithText("Follow").performClick()
        assert(followed)
    }

    @Test
    fun `following state shows remaining distance from the fix`() {
        composeRule.setContent {
            RouteCard(
                state = RouteUiState.Following(plan),
                preset = RoutePreset.Balanced,
                userLocation = LatLng(69.0, 18.0), // at the start → ~full distance remaining
                onSelectPreset = {}, onFollow = {}, onSave = {}, onClear = {},
            )
        }
        composeRule.onNodeWithText("Following route").assertExists()
        composeRule.onNodeWithText("Stop").assertExists()
        composeRule.onNodeWithText("km left", substring = true).assertExists()
    }

    @Test
    fun `error state shows the message and dismiss`() {
        composeRule.setContent {
            RouteCard(
                state = RouteUiState.Error("No route found"),
                preset = RoutePreset.Balanced,
                userLocation = null,
                onSelectPreset = {}, onFollow = {}, onSave = {}, onClear = {},
            )
        }
        composeRule.onNodeWithText("No route found").assertExists()
        composeRule.onNodeWithText("Dismiss").assertExists()
    }
}
