package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.ui.unit.Density
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.map.MapHostCoordinator.CameraRestore
import com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure-logic guards for the map host's decisions (see [MapHostCoordinator]). */
class MapHostCoordinatorTest {

    private val line = listOf(LatLng(60.0, 10.0), LatLng(60.0, 10.001))
    private val fallback = LatLng(64.5, 13.5)

    @Test fun `off-route is false for a point on the line`() {
        assertFalse(MapHostCoordinator.isOffRoute(line, LatLng(60.0, 10.0005)))
    }

    @Test fun `off-route is true for a point a kilometre away`() {
        assertTrue(MapHostCoordinator.isOffRoute(line, LatLng(60.01, 10.0)))
    }

    @Test fun `off-route needs at least two points`() {
        assertFalse(MapHostCoordinator.isOffRoute(listOf(LatLng(60.0, 10.0)), LatLng(61.0, 11.0)))
    }

    @Test fun `the live sheet never hides the user's location dot`() {
        val d = Density(2f)
        val screen = 1000f
        // The reserved inset keeps the dot in the band above the sheet: at every
        // detent — even a full-height sheet — it stays at or under half the screen,
        // so "centre on me" never parks the dot behind the sheet.
        for (detent in LiveDetent.entries) {
            val inset = MapHostCoordinator.bottomInsetPx(detent, screen, d)
            assertTrue("detent $detent reserved $inset px — must leave the dot visible (<= ${screen / 2})", inset <= screen / 2)
        }
    }

    @Test fun `camera restore - none once already centred or focus pending`() {
        assertEquals(
            CameraRestore.None,
            MapHostCoordinator.cameraRestore(true, false, LatLng(60.0, 10.0), 12.0, null, false, false),
        )
        assertEquals(
            CameraRestore.None,
            MapHostCoordinator.cameraRestore(false, true, LatLng(60.0, 10.0), 12.0, null, false, false),
        )
    }

    @Test fun `camera restore - saved camera wins, default zoom when absent`() {
        val cam = LatLng(59.9, 10.7)
        assertEquals(
            CameraRestore.RestoreSaved(cam, 12.0),
            MapHostCoordinator.cameraRestore(false, false, cam, 12.0, LatLng(1.0, 1.0), false, false),
        )
        assertEquals(
            CameraRestore.RestoreSaved(cam, MapHostCoordinator.INITIAL_LOCATION_ZOOM),
            MapHostCoordinator.cameraRestore(false, false, cam, null, null, false, false),
        )
    }

    @Test fun `camera restore - first fix only when idle and no saved camera`() {
        val here = LatLng(63.4, 10.4)
        assertEquals(
            CameraRestore.FlyToFix(here, MapHostCoordinator.INITIAL_LOCATION_ZOOM),
            MapHostCoordinator.cameraRestore(false, false, null, null, here, false, false),
        )
        // Following or recording own the camera → don't fight them.
        assertEquals(
            CameraRestore.None,
            MapHostCoordinator.cameraRestore(false, false, null, null, here, true, false),
        )
        assertEquals(
            CameraRestore.None,
            MapHostCoordinator.cameraRestore(false, false, null, null, here, false, true),
        )
    }

    @Test fun `persist camera - skips unchanged, fallback and null-island`() {
        val cur = Triple(60.0, 10.0, 14.0)
        assertFalse(MapHostCoordinator.shouldPersistCamera(cur, cur, fallback))
        assertFalse(MapHostCoordinator.shouldPersistCamera(Triple(64.5, 13.5, 5.0), null, fallback))
        assertFalse(MapHostCoordinator.shouldPersistCamera(Triple(0.0, 0.0, 14.0), null, fallback))
        assertTrue(MapHostCoordinator.shouldPersistCamera(cur, Triple(60.1, 10.1, 14.0), fallback))
    }
}
