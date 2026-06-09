package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import org.junit.Assert.assertTrue
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

    @Test
    fun `within limits enables download and confirms`() {
        var confirmed = false
        composeRule.setContent {
            DownloadAreaDialog(
                estimate = OfflineEstimate(tiles = 1_200, bytes = 24_000_000, withinLimits = true),
                onConfirm = { confirmed = true },
                onDismiss = {},
            )
        }
        composeRule.onNodeWithTag("downloadConfirm").assertIsEnabled().performClick()
        assertTrue(confirmed)
    }

    @Test
    fun `an over-limit area disables download and explains why`() {
        composeRule.setContent {
            DownloadAreaDialog(
                estimate = OfflineEstimate(tiles = 999_999, bytes = 2_000_000_000, withinLimits = false),
                onConfirm = {},
                onDismiss = {},
            )
        }
        composeRule.onNodeWithTag("downloadConfirm").assertIsNotEnabled()
        composeRule.onNodeWithText("Zoom in", substring = true).assertExists()
    }
}
