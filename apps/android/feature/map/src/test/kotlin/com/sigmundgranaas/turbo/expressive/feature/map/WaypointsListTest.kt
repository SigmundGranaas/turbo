package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.WaypointsList

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.domain.LatLng
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
class WaypointsListTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val stops = listOf(
        LatLng(69.60, 18.90), // start
        LatLng(69.62, 18.95), // via 1
        LatLng(69.64, 19.00), // via 2
        LatLng(69.66, 19.05), // destination
    )

    private fun list(
        onMove: (Int, Int) -> Unit = { _, _ -> },
        onRemove: (Int) -> Unit = {},
        onAddStop: () -> Unit = {},
    ) {
        composeRule.setContent {
            MaterialTheme {
                WaypointsList(stops, statText = "8.4 km · ↑ 320 m", onMove = onMove, onRemove = onRemove, onAddStop = onAddStop)
            }
        }
    }

    @Test
    fun `renders a row per stop plus the stat line and add-stop`() {
        list()
        composeRule.onNodeWithTag("stopsStat").assertIsDisplayed()
        composeRule.onNodeWithTag("wpRow_0").assertIsDisplayed()
        composeRule.onNodeWithTag("wpRow_3").assertIsDisplayed()
        composeRule.onNodeWithTag("addStop").assertIsDisplayed()
    }

    @Test
    fun `only intermediate stops expose a remove button`() {
        var removed: Int? = null
        list(onRemove = { removed = it })
        composeRule.onNodeWithTag("wpRemove_0").assertDoesNotExist()
        composeRule.onNodeWithTag("wpRemove_3").assertDoesNotExist()
        composeRule.onNodeWithTag("wpRemove_1").assertIsDisplayed().performClick()
        assertEquals(1, removed)
    }

    @Test
    fun `add stop fires its callback`() {
        var added = false
        list(onAddStop = { added = true })
        composeRule.onNodeWithTag("addStop").performClick()
        assertEquals(true, added)
    }

    @Test
    fun `move-down button shifts a stop one place later`() {
        var move: Pair<Int, Int>? = null
        list(onMove = { from, to -> move = from to to })
        composeRule.onNodeWithTag("wpDown_1").performClick()
        assertEquals(1 to 2, move)
    }

    @Test
    fun `move-up button shifts a stop one place earlier`() {
        var move: Pair<Int, Int>? = null
        list(onMove = { from, to -> move = from to to })
        composeRule.onNodeWithTag("wpUp_2").performClick()
        assertEquals(2 to 1, move)
    }

    @Test
    fun `the ends cannot move past the edge`() {
        list()
        composeRule.onNodeWithTag("wpUp_0").assertIsNotEnabled()       // start can't go up
        composeRule.onNodeWithTag("wpDown_3").assertIsNotEnabled()     // destination can't go down
    }
}
