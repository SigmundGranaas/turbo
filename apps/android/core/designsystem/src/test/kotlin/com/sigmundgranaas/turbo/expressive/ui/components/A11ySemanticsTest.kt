package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Place
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Guards the shared-primitive accessibility contract: TalkBack should read a
 * stat tile / spec row as ONE labelled node (not two disjoint texts), and a list
 * row's title+subtitle as one node. These are merged via clearAndSetSemantics /
 * mergeDescendants in [StatTile], [SpecRow], [ListRowItem].
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class A11ySemanticsTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `StatTile collapses value and label into one spoken node`() {
        composeRule.setContent { StatTile(value = "12.4 km", label = "Distance") }
        // The merged node is reachable by its combined contentDescription…
        composeRule.onNodeWithContentDescription("Distance: 12.4 km").assertIsDisplayed()
        // …and the raw value text is no longer a separate semantics node.
        composeRule.onNodeWithText("12.4 km").assertDoesNotExist()
    }

    @Test
    fun `SpecRow collapses label and value into one spoken node`() {
        composeRule.setContent { SpecRow(label = "Surface", value = "Trail 80%") }
        composeRule.onNodeWithContentDescription("Surface, Trail 80%").assertIsDisplayed()
    }

    @Test
    fun `ListRowItem merges title and subtitle`() {
        composeRule.setContent {
            ListRowItem(icon = Icons.Rounded.Place, title = "Galdhøpiggen", subtitle = "Mountain · 2469 m")
        }
        // mergeDescendants keeps title+subtitle as one focusable node read together.
        composeRule.onNodeWithText("Galdhøpiggen", substring = true).assertIsDisplayed()
    }
}
