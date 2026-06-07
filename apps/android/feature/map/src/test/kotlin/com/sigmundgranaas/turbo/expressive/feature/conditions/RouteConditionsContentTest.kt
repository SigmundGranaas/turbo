package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class RouteConditionsContentTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `content shows the temp range and avalanche badge`() {
        composeRule.setContent {
            RouteConditionsContent(
                RouteConditionsUiState.Content(
                    RouteConditions(tempMinC = -2.0, tempMaxC = 5.0, worstDanger = 3, samples = 4),
                ),
            )
        }
        composeRule.onNodeWithTag("routeCond").assertIsDisplayed()
        composeRule.onNodeWithText("-2° to 5°").assertExists()
        composeRule.onNodeWithTag("routeCondAvalanche").assertIsDisplayed()
    }

    @Test
    fun `no avalanche badge when no danger reported`() {
        composeRule.setContent {
            RouteConditionsContent(
                RouteConditionsUiState.Content(
                    RouteConditions(tempMinC = 4.0, tempMaxC = 4.0, worstDanger = null, samples = 4),
                ),
            )
        }
        composeRule.onNodeWithTag("routeCondTemp").assertExists()
        composeRule.onNodeWithText("4°").assertExists()
        composeRule.onNodeWithTag("routeCondAvalanche").assertDoesNotExist()
    }

    @Test
    fun `loading shows the checking line`() {
        composeRule.setContent { RouteConditionsContent(RouteConditionsUiState.Loading) }
        composeRule.onNodeWithTag("routeCondLoading").assertIsDisplayed()
    }

    @Test
    fun `error and idle render nothing`() {
        composeRule.setContent { RouteConditionsContent(RouteConditionsUiState.Error) }
        composeRule.onNodeWithTag("routeCond").assertDoesNotExist()
        composeRule.onNodeWithTag("routeCondLoading").assertDoesNotExist()
    }
}
