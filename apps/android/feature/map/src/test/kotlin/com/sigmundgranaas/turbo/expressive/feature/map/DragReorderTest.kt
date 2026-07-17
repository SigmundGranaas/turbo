package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.dragReorderTarget
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The pure drag-to-reorder target: a stop dragged by N row-heights lands N slots away, rounded
 * and clamped into the list. This is the whole judgement of the stops drag handle — the Compose
 * gesture just feeds it a live delta and commits the result. No device needed.
 */
class DragReorderTest {

    private val rowPx = 56f

    @Test
    fun `dragging down one row moves the stop one slot later`() {
        assertEquals(3, dragReorderTarget(from = 2, dyPx = rowPx, rowHeightPx = rowPx, count = 5))
    }

    @Test
    fun `dragging up two rows moves two slots earlier`() {
        assertEquals(1, dragReorderTarget(from = 3, dyPx = -2 * rowPx, rowHeightPx = rowPx, count = 5))
    }

    @Test
    fun `a half-row nudge rounds to the nearest slot`() {
        assertEquals(2, dragReorderTarget(from = 2, dyPx = rowPx * 0.4f, rowHeightPx = rowPx, count = 5)) // stays
        assertEquals(3, dragReorderTarget(from = 2, dyPx = rowPx * 0.6f, rowHeightPx = rowPx, count = 5)) // tips over
    }

    @Test
    fun `the target clamps to the ends of the list`() {
        assertEquals(0, dragReorderTarget(from = 1, dyPx = -10 * rowPx, rowHeightPx = rowPx, count = 4))
        assertEquals(3, dragReorderTarget(from = 1, dyPx = 10 * rowPx, rowHeightPx = rowPx, count = 4))
    }

    @Test
    fun `degenerate inputs are no-ops`() {
        assertEquals(2, dragReorderTarget(from = 2, dyPx = 500f, rowHeightPx = rowPx, count = 1)) // single item
        assertEquals(2, dragReorderTarget(from = 2, dyPx = 500f, rowHeightPx = 0f, count = 5)) // zero row height
    }
}
