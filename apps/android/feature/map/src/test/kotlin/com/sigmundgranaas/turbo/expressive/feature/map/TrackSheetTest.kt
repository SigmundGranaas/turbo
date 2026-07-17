package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.DragDirection
import com.sigmundgranaas.turbo.expressive.feature.map.route.TrackDetent
import com.sigmundgranaas.turbo.expressive.feature.map.route.dragDirection
import com.sigmundgranaas.turbo.expressive.feature.map.route.nextDetent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The create-track sheet's detent state machine — the pure seam the grabber drives, since
 * a Compose drag can't be adb-scripted. Dragging the handle walks the sheet through every
 * stop and sticks at the extremes; a tiny wobble under the slop changes nothing.
 */
class TrackSheetTest {

    @Test
    fun `dragging up steps one stop taller, down steps one shorter`() {
        assertEquals(TrackDetent.Default, nextDetent(TrackDetent.Collapsed, DragDirection.Up))
        assertEquals(TrackDetent.Expanded, nextDetent(TrackDetent.Default, DragDirection.Up))
        assertEquals(TrackDetent.Default, nextDetent(TrackDetent.Expanded, DragDirection.Down))
        assertEquals(TrackDetent.Collapsed, nextDetent(TrackDetent.Default, DragDirection.Down))
    }

    @Test
    fun `the ends clamp — you can't drag past the tallest or shortest stop`() {
        assertEquals(TrackDetent.Expanded, nextDetent(TrackDetent.Expanded, DragDirection.Up))
        assertEquals(TrackDetent.Collapsed, nextDetent(TrackDetent.Collapsed, DragDirection.Down))
    }

    @Test
    fun `repeated drags walk through every stop in order`() {
        var d = TrackDetent.Collapsed
        d = nextDetent(d, DragDirection.Up); assertEquals(TrackDetent.Default, d)
        d = nextDetent(d, DragDirection.Up); assertEquals(TrackDetent.Expanded, d)
        d = nextDetent(d, DragDirection.Up); assertEquals(TrackDetent.Expanded, d) // clamped
    }

    @Test
    fun `a drag maps to a direction only past the slop`() {
        assertEquals(DragDirection.Up, dragDirection(-40f, thresholdPx = 24f))     // screen-up = grow
        assertEquals(DragDirection.Down, dragDirection(40f, thresholdPx = 24f))    // screen-down = shrink
        assertNull("a wobble under the slop changes nothing", dragDirection(10f, thresholdPx = 24f))
        assertNull(dragDirection(-10f, thresholdPx = 24f))
    }

    @Test
    fun `taller stops reserve more of the screen`() {
        assertTrue(TrackDetent.Collapsed.heightFraction < TrackDetent.Default.heightFraction)
        assertTrue(TrackDetent.Default.heightFraction < TrackDetent.Expanded.heightFraction)
    }
}
