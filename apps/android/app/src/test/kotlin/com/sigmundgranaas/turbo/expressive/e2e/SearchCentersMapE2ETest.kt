package com.sigmundgranaas.turbo.expressive.e2e

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.sigmundgranaas.turbo.expressive.HiltTestActivity
import com.sigmundgranaas.turbo.expressive.feature.map.core.LocalMapEngineOverride
import com.sigmundgranaas.turbo.expressive.ui.nav.TurboNavGraph
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboTheme
import dagger.hilt.android.testing.HiltAndroidRule
import dagger.hilt.android.testing.HiltAndroidTest
import dagger.hilt.android.testing.HiltTestApplication
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * User goal: "I search for a place and the map takes me there."
 *
 * Drives the real app across two screens (home map → search → back) and asserts
 * the *observable outcome* — the map camera moved to the picked place — via the
 * recording [FakeMapEngine]. No mock-verify-was-called: the assertion is the
 * place the user ended up looking at.
 */
@HiltAndroidTest
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(application = HiltTestApplication::class, sdk = [34])
class SearchCentersMapE2ETest {

    @get:Rule(order = 0)
    val hilt = HiltAndroidRule(this)

    @get:Rule(order = 1)
    val compose = createAndroidComposeRule<HiltTestActivity>()

    private val mapEngine = FakeMapEngine()

    @Before
    fun setUp() {
        hilt.inject()
        compose.setContent {
            TurboTheme {
                CompositionLocalProvider(LocalMapEngineOverride provides mapEngine) {
                    TurboNavGraph()
                }
            }
        }
    }

    @Test
    fun `searching for a place centres the map on it`() {
        // From the home map, open search.
        compose.onNodeWithText("Search places, coordinates…").performClick()

        // Type a query; the synthetic backend returns "Bodøfjellet" at 69.65, 18.95.
        compose.onNode(hasSetTextAction()).performTextInput("Bodø")

        // Wait out the search debounce until the result appears, then pick it.
        compose.waitUntil(timeoutMillis = 5_000) {
            compose.onAllNodesWithText("Bodøfjellet").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Bodøfjellet").performClick()

        // The user is back on the map, now looking at the place they picked.
        compose.waitForIdle()
        val flewTo = mapEngine.lastFlyTo
        assertNotNull("expected the map to centre on the picked place", flewTo)
        assertEquals(69.65, flewTo!!.lat, 0.01)
        assertEquals(18.95, flewTo.lng, 0.01)
    }
}
