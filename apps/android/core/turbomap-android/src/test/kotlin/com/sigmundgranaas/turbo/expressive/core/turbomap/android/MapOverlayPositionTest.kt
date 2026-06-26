package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The live-position indicator must always be discoverable. When the fix projects inside the
 * viewport it's the [myPosition] dot; when it projects OUTSIDE (the common case on open, because
 * the app restores the last camera rather than recentring) it must become the edge chevron
 * ([myPositionOffscreen]) instead of being placed off-screen and vanishing — the "I can't see it"
 * bug.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class MapOverlayPositionTest {

    @get:Rule
    val composeRule = createComposeRule()

    /** A MapEngine whose only meaningful behaviour is a fixed [toScreen] projection. */
    private class FakeEngine(val screen: Pair<Float, Float>) : MapEngine {
        override fun toScreen(point: LatLng): Pair<Float, Float> = screen
        override fun fromScreen(xPx: Float, yPx: Float): LatLng = LatLng(0.0, 0.0)
        override fun screenToGround(xPx: Float, yPx: Float): LatLng = LatLng(0.0, 0.0)
        override fun center(): LatLng = LatLng(0.0, 0.0)
        override fun visibleBounds(): GeoBounds = GeoBounds(0.0, 0.0, 0.0, 0.0)
        override fun zoom(): Double = 12.0
        override fun bearing(): Double = 0.0
        override fun zoomIn() {}
        override fun zoomOut() {}
        override fun flyTo(target: LatLng, zoom: Double) {}
        override fun setBottomInset(bottomPx: Int) {}
        override fun resetNorth() {}
        override fun frameTo(points: List<LatLng>, paddingPx: Int) {}
    }

    private fun overlay(engine: MapEngine) {
        composeRule.setContent {
            MaterialTheme {
                MapOverlay(
                    engine = engine,
                    cameraTick = 1,
                    modifier = Modifier.fillMaxSize(),
                    userLocation = LatLng(67.0, 15.0),
                )
            }
        }
    }

    @Test
    fun `position inside the viewport shows the dot`() {
        overlay(FakeEngine(120f to 220f))
        composeRule.onNodeWithTag("myPosition").assertIsDisplayed()
        composeRule.onNodeWithTag("myPositionOffscreen").assertDoesNotExist()
    }

    @Test
    fun `position outside the viewport shows the edge chevron instead of vanishing`() {
        // Far off the right/bottom of any plausible viewport → must clamp to a visible chevron.
        overlay(FakeEngine(100_000f to 100_000f))
        composeRule.onNodeWithTag("myPositionOffscreen").assertIsDisplayed()
        composeRule.onNodeWithTag("myPosition").assertDoesNotExist()
    }
}
