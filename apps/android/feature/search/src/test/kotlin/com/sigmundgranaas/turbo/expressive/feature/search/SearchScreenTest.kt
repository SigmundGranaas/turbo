package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class StubSearchRepository(private val hits: List<SearchHit>) : SearchRepository {
    override suspend fun search(query: String): Outcome<List<SearchHit>> = Outcome.Success(hits)
}

private class StubMarkerRepository : MarkerRepository {
    override fun observeAll(): Flow<List<Marker>> = flowOf(emptyList())
    override suspend fun upsert(marker: Marker) = Unit
    override suspend fun delete(id: String) = Unit
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SearchScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val hit = SearchHit("Lyngen", "Troms", LatLng(69.6, 20.0))

    @Test
    fun `typing shows results and tapping one returns its coordinates`() {
        var picked: Triple<Double, Double, String>? = null
        composeRule.setContent {
            SearchScreen(
                onBack = {},
                onPick = { lat, lng, name -> picked = Triple(lat, lng, name) },
                viewModel = SearchViewModel(StubSearchRepository(listOf(hit)), StubMarkerRepository()),
            )
        }

        composeRule.onNode(hasSetTextAction()).performTextInput("Lyn")
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Lyngen").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Lyngen").performClick()

        assertEquals(69.6, picked!!.first, 1e-9)
        assertEquals(20.0, picked!!.second, 1e-9)
        assertEquals("Lyngen", picked!!.third)
    }

    @Test
    fun `clear resets to the empty hint`() {
        composeRule.setContent {
            SearchScreen(
                onBack = {},
                onPick = { _, _, _ -> },
                viewModel = SearchViewModel(StubSearchRepository(listOf(hit)), StubMarkerRepository()),
            )
        }
        composeRule.onNode(hasSetTextAction()).performTextInput("Lyn")
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodesWithText("Lyngen").fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithContentDescription("Clear").performClick()

        composeRule.onNodeWithText("Search Kartverket place names").assertIsDisplayed()
    }
}
