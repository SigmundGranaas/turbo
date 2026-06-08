package com.sigmundgranaas.turbo.expressive.ui.map

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Drives the on-map waypoint marker's gestures headlessly (the projection is faked, so no
 * MapLibre needed): tap = select, long-press = remove, and dragging any stop = move.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class WaypointMarkerTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun marker(
        selected: Boolean = false,
        onTap: () -> Unit = {},
        onLongPress: () -> Unit = {},
        onMoved: (LatLng) -> Unit = {},
    ) {
        composeRule.setContent {
            MaterialTheme {
                WaypointMarkerView(
                    index = 1,
                    last = 2,
                    selected = selected,
                    cameraTick = 0,
                    project = { Offset(200f, 200f) },
                    // Fake screen→world: lng = x, lat = y, so we can assert the drag landed.
                    toLatLng = { o -> LatLng(o.y.toDouble(), o.x.toDouble()) },
                    onTap = onTap,
                    onLongPress = onLongPress,
                    onMoved = onMoved,
                )
            }
        }
    }

    @Test
    fun `tap selects (fires onTap)`() {
        var tapped = false
        marker(onTap = { tapped = true })
        composeRule.onNodeWithTag("waypoint_1").performClick()
        assertTrue(tapped)
    }

    @Test
    fun `long-press removes (fires onLongPress)`() {
        var removed = false
        marker(onLongPress = { removed = true })
        composeRule.onNodeWithTag("waypoint_1").performTouchInput { longClick() }
        assertTrue(removed)
    }

    @Test
    fun `dragging the selected stop moves it (fires onMoved with the dropped position)`() {
        var moved: LatLng? = null
        marker(selected = true, onMoved = { moved = it })
        composeRule.onNodeWithTag("waypoint_1").performTouchInput {
            swipe(start = center, end = center + Offset(120f, 80f), durationMillis = 200)
        }
        val drop = moved
        assertNotNull("a drag on the selected stop should commit a move", drop)
        // base (200,200) + drag(120,80) → toLatLng(x=320,y=280) = lat 280, lng 320.
        assertEquals(320.0, drop!!.lng, 40.0)
        assertEquals(280.0, drop.lat, 40.0)
    }

    @Test
    fun `an unselected stop is also directly draggable (no select-first)`() {
        var moved: LatLng? = null
        marker(selected = false, onMoved = { moved = it })
        composeRule.onNodeWithTag("waypoint_1").performTouchInput {
            swipe(start = center, end = center + Offset(120f, 80f), durationMillis = 200)
        }
        val drop = moved
        assertNotNull("dragging any stop should commit a move without selecting first", drop)
        assertEquals(320.0, drop!!.lng, 40.0)
        assertEquals(280.0, drop.lat, 40.0)
    }
}
