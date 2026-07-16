package com.sigmundgranaas.turbo.expressive.ui.layout

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The rail's collision/auto-hide behaviour, as a user would see it: buttons
 * never overlap the search bar, they sit near the bottom, and the +/− zoom
 * buttons are the first to disappear when a growing sheet steals their space —
 * essentials (compass/layers/location) always stay reachable. Driven purely
 * (band + item heights → placements), so it needs no device.
 */
class RailLayoutTest {

    // Five items in visual order: compass, layers, location (essential) + zoom
    // in/out (non-essential). 52px buttons, 10px spacing — the shipped sizes.
    private val h = 52f
    private val spacing = 10f
    private val items = listOf(
        RailItem("compass", h, essential = true),
        RailItem("layers", h, essential = true),
        RailItem("location", h, essential = true),
        RailItem("zoomIn", h, essential = false),
        RailItem("zoomOut", h, essential = false),
    )

    private fun placement(list: List<RailPlacement>, id: String) = list.first { it.id == id }

    @Test
    fun `with room, all five show, in order, none over the search bar`() {
        val out = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 800f, spacingPx = spacing)
        assertTrue("all visible", out.all { it.visible })
        // Never overlaps the search bar: the topmost visible top >= topBound.
        assertTrue("clears the search bar", out.filter { it.visible }.minOf { it.topPx } >= 100f)
        // Order preserved top→bottom.
        val tops = out.map { it.topPx }
        assertEquals(tops.sortedBy { it }, tops)
    }

    @Test
    fun `it bottom-anchors — the last button sits at the band's bottom`() {
        val out = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 800f, spacingPx = spacing)
        val last = placement(out, "zoomOut")
        assertEquals(800f - h, last.topPx, 0.01f) // its top is one button-height off the bottom
    }

    @Test
    fun `a tall sheet hides the zoom buttons first, keeps the essentials`() {
        // Band tall enough for the three essentials + spacing, but not the two zoom buttons.
        // 3*52 + 2*10 = 176; give ~190px.
        val out = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 290f, spacingPx = spacing)
        assertTrue("compass stays", placement(out, "compass").visible)
        assertTrue("layers stays", placement(out, "layers").visible)
        assertTrue("location stays", placement(out, "location").visible)
        assertFalse("zoom-in hides", placement(out, "zoomIn").visible)
        assertFalse("zoom-out hides", placement(out, "zoomOut").visible)
        // Still never over the search bar.
        assertTrue(out.filter { it.visible }.minOf { it.topPx } >= 100f)
    }

    @Test
    fun `only one zoom button drops when only one extra fits`() {
        // Room for the three essentials + exactly one more: 4*52 + 3*10 = 238; give ~245.
        val out = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 345f, spacingPx = spacing)
        assertTrue(placement(out, "zoomIn").visible) // the earlier-listed extra survives
        assertFalse(placement(out, "zoomOut").visible) // the last-listed drops first
    }

    @Test
    fun `restoring space brings the zoom buttons back`() {
        val tight = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 290f, spacingPx = spacing)
        assertFalse(placement(tight, "zoomOut").visible)
        val roomy = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 800f, spacingPx = spacing)
        assertTrue(placement(roomy, "zoomOut").visible)
    }

    @Test
    fun `essentials never overlap the search bar even when the band is too short for them`() {
        // Degenerate: not even the three essentials fit. They stay visible (can't
        // drop essentials) but must still clamp below the search bar, not above it.
        val out = layoutRail(items, topBoundPx = 100f, bottomBoundPx = 220f, spacingPx = spacing)
        assertTrue(out.filter { it.visible }.all { it.topPx >= 100f })
    }
}
