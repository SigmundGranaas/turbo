package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.domain.DetailLevel
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
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
class DownloadAreaDialogTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val small = OfflineEstimate(tiles = 1_200, bytes = 24_000_000, withinLimits = true)
    private val huge = OfflineEstimate(tiles = 999_999, bytes = 2_000_000_000, withinLimits = false)

    @Test
    fun `within limits enables download and confirms with the chosen detail`() {
        var confirmed: DetailLevel? = null
        composeRule.setContent {
            DownloadAreaDialog(estimateFor = { small }, onConfirm = { confirmed = it }, onDismiss = {})
        }
        composeRule.onNodeWithTag("downloadConfirm").assertIsEnabled().performClick()
        assertEquals(DetailLevel.Standard, confirmed)
    }

    @Test
    fun `switching to Detailed re-estimates and confirms Detailed`() {
        var confirmed: DetailLevel? = null
        composeRule.setContent {
            DownloadAreaDialog(
                // Detailed pushes this area over the limit; Standard stays fine.
                estimateFor = { if (it == DetailLevel.Detailed) huge else small },
                onConfirm = { confirmed = it },
                onDismiss = {},
            )
        }
        composeRule.onNodeWithTag("downloadConfirm").assertIsEnabled()
        composeRule.onNodeWithTag("detail_Detailed").performClick()
        composeRule.onNodeWithTag("downloadConfirm").assertIsNotEnabled()
        composeRule.onNodeWithTag("detail_Standard").performClick()
        composeRule.onNodeWithTag("downloadConfirm").assertIsEnabled().performClick()
        assertEquals(DetailLevel.Standard, confirmed)
    }

    @Test
    fun `an over-limit area disables download and explains why`() {
        composeRule.setContent {
            DownloadAreaDialog(estimateFor = { huge }, onConfirm = {}, onDismiss = {})
        }
        composeRule.onNodeWithTag("downloadConfirm").assertIsNotEnabled()
        composeRule.onNodeWithText("Zoom in", substring = true).assertExists()
    }
}
