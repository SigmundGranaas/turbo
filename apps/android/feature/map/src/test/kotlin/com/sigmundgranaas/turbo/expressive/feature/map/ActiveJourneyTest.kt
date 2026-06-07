package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.data.RecordingSession
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ActiveJourneyTest {

    private val a = LatLng(69.6, 18.9)
    private val b = LatLng(69.7, 19.0)
    private val plan = RoutePlan(
        distanceM = 1200.0, durationS = 900.0, ascentM = 80.0,
        onTrailPct = 0.8, surfaces = emptyMap(), geometry = listOf(a, b),
    )

    @Test
    fun `idle when nothing is happening`() {
        val j = resolveJourney(RouteUiState.Idle, RecordingSession())
        assertEquals(JourneyMode.Idle, j.mode)
        assertFalse(j.isActive)
    }

    @Test
    fun `solving is a planning journey showing the progress polyline`() {
        val j = resolveJourney(RouteUiState.Solving(listOf(a, b)), RecordingSession())
        assertEquals(JourneyMode.Planning, j.mode)
        assertEquals(listOf(a, b), j.geometry)
    }

    @Test
    fun `done carries plan stats`() {
        val j = resolveJourney(RouteUiState.Done(plan), RecordingSession())
        assertEquals(JourneyMode.Planning, j.mode)
        assertEquals(1200.0, j.distanceM, 0.0)
        assertEquals(80.0, j.ascentM!!, 0.0)
    }

    @Test
    fun `following carries plan geometry + duration`() {
        val j = resolveJourney(RouteUiState.Following(plan), RecordingSession())
        assertEquals(JourneyMode.Following, j.mode)
        assertEquals(900.0, j.durationS!!, 0.0)
    }

    @Test
    fun `an active recording wins over any route state`() {
        val rec = RecordingSession(active = true, paused = true, points = listOf(a, b), distanceM = 42.0, elapsedSec = 30)
        val j = resolveJourney(RouteUiState.Following(plan), rec)
        assertEquals(JourneyMode.Recording, j.mode)
        assertEquals(42.0, j.distanceM, 0.0)
        assertEquals(30, j.elapsedSec)
        assertTrue(j.paused)
        assertEquals(listOf(a, b), j.geometry)
    }
}
