package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.RouteCard
import com.sigmundgranaas.turbo.expressive.feature.map.route.RouteUiState

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
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
            RouteCard(state = RouteUiState.Done(plan), onFollow = {}, onSave = {}, onClear = {})
        }
        composeRule.onNodeWithText("8.5 km").assertExists()
        composeRule.onNodeWithText("Follow").assertExists()
        composeRule.onNodeWithText("Clear").assertExists()
    }

    @Test
    fun `the card exposes no route-style selector (Phase 4 removed it)`() {
        composeRule.setContent {
            RouteCard(state = RouteUiState.Done(plan), onFollow = {}, onSave = {}, onClear = {})
        }
        // The preset chips are gone; solving still defaults to Balanced under the hood.
        composeRule.onNodeWithText("Avoid roads").assertDoesNotExist()
        composeRule.onNodeWithText("Balanced").assertDoesNotExist()
        composeRule.onNodeWithText("Trail purist").assertDoesNotExist()
    }

    @Test
    fun `tapping Follow starts following the route`() {
        var followed = false
        composeRule.setContent {
            RouteCard(state = RouteUiState.Done(plan), onFollow = { followed = true }, onSave = {}, onClear = {})
        }
        composeRule.onNodeWithText("Follow").performClick()
        assert(followed)
    }

    // Following is rendered by LiveSheet now (see LiveSheetTest), not RouteCard — the
    // card's Following branch was removed, so its old follow-card tests went with it.

    @Test
    fun `error state shows the message and dismiss`() {
        composeRule.setContent {
            RouteCard(state = RouteUiState.Error("No route found"), onFollow = {}, onSave = {}, onClear = {})
        }
        composeRule.onNodeWithText("No route found").assertExists()
        composeRule.onNodeWithText("Dismiss").assertExists()
    }
}
