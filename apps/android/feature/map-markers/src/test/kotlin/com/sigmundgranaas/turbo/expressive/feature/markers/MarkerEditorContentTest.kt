package com.sigmundgranaas.turbo.expressive.feature.markers

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
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
class MarkerEditorContentTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val point = LatLng(69.6, 18.9)

    @Test
    fun `save is disabled until a name is entered, then returns it`() {
        var saved: Triple<String, ActivityKindId, String?>? = null
        composeRule.setContent {
            MaterialTheme {
                MarkerEditorContent(point, existing = null) { name, kind, _, notes ->
                    saved = Triple(name, kind, notes)
                }
            }
        }

        composeRule.onNodeWithTag("markerSave").performScrollTo().assertIsNotEnabled()
        composeRule.onNodeWithTag("markerName").performTextInput("Secret tarn")
        composeRule.onNodeWithTag("markerNotes").performTextInput("great swim")
        composeRule.onNodeWithTag("markerSave").performScrollTo().assertIsEnabled().performClick()

        assertEquals("Secret tarn", saved?.first)
        assertEquals("great swim", saved?.third)
    }

    @Test
    fun `edit mode prefills name and notes and updates`() {
        val existing = Marker("m1", "Old cabin", ActivityKindId.Cabin, point, colorArgb = null, notes = "leaky roof")
        var savedName: String? = null
        composeRule.setContent {
            MaterialTheme {
                MarkerEditorContent(point, existing = existing) { name, _, _, _ -> savedName = name }
            }
        }

        composeRule.onNodeWithTag("markerName").performTextReplacement("Fixed cabin")
        composeRule.onNodeWithTag("markerSave").performScrollTo().performClick()

        assertEquals("Fixed cabin", savedName)
    }
}
