package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The map-point card's tap semantics as a user experiences them: empty tap opens
 * it, an entity tap yields to the entity, a second tap re-anchors (not dismiss),
 * long-press drops on top of an entity, panning dismisses, and track mode
 * suppresses it entirely. Pure reducer — the whole point is testing the tap
 * behaviour without Compose or a device.
 */
class MapPointCardTest {

    private val a = LatLng(69.96, 23.27)
    private val b = LatLng(69.97, 23.30)

    private fun reduce(state: MapPointCard, event: MapPointCardEvent, track: Boolean = false) =
        reduceMapPointCard(state, event, track)

    @Test
    fun `an empty-map tap opens the card at the point`() {
        val out = reduce(MapPointCard.Hidden, MapPointCardEvent.Tap(a, onEntity = false))
        assertEquals(MapPointCard.Shown(a, expanded = false), out)
    }

    @Test
    fun `tapping an entity yields — the card stays hidden so the entity detail wins`() {
        val out = reduce(MapPointCard.Hidden, MapPointCardEvent.Tap(a, onEntity = true))
        assertEquals(MapPointCard.Hidden, out)
    }

    @Test
    fun `a tap while open re-anchors to the new point and collapses the expansion`() {
        val open = MapPointCard.Shown(a, expanded = true)
        val out = reduce(open, MapPointCardEvent.Tap(b, onEntity = false))
        assertEquals(MapPointCard.Shown(b, expanded = false), out)
    }

    @Test
    fun `long-press opens the card even over an entity`() {
        val out = reduce(MapPointCard.Hidden, MapPointCardEvent.LongPress(a))
        assertEquals(MapPointCard.Shown(a, expanded = false), out)
    }

    @Test
    fun `panning dismisses the card`() {
        val out = reduce(MapPointCard.Shown(a), MapPointCardEvent.Pan)
        assertEquals(MapPointCard.Hidden, out)
    }

    @Test
    fun `track mode suppresses the card — taps place points instead`() {
        assertEquals(MapPointCard.Hidden, reduce(MapPointCard.Hidden, MapPointCardEvent.Tap(a, onEntity = false), track = true))
        assertEquals(MapPointCard.Hidden, reduce(MapPointCard.Hidden, MapPointCardEvent.LongPress(a), track = true))
    }

    @Test
    fun `toggling Add Marker expands and collapses only while shown`() {
        val open = MapPointCard.Shown(a, expanded = false)
        val expanded = reduce(open, MapPointCardEvent.ToggleAddMarker)
        assertEquals(MapPointCard.Shown(a, expanded = true), expanded)
        assertEquals(MapPointCard.Shown(a, expanded = false), reduce(expanded, MapPointCardEvent.ToggleAddMarker))
        // No-op while hidden.
        assertEquals(MapPointCard.Hidden, reduce(MapPointCard.Hidden, MapPointCardEvent.ToggleAddMarker))
    }
}
