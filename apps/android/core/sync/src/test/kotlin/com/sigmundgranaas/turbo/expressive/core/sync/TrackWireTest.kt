package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pins the wire contract against the .NET backend's track DTOs + the entity mapper. */
class TrackWireTest {

    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false }

    @Test
    fun `parses a tracks delta response with items and tombstones`() {
        val payload = """
            {
              "items": [{
                "id": "srv-1",
                "geometry": {"points": [{"longitude": 10.5, "latitude": 60.2}, {"longitude": 10.6, "latitude": 60.3}], "elevations": [100.0, 110.0]},
                "metadata": {"name": "Morning loop"},
                "stats": {"distanceMeters": 2143.0, "ascentMeters": 120.0, "movingTimeSeconds": 1800, "recordedAt": "2024-01-01T12:00:00Z"},
                "createdAt": "2024-01-01T10:00:00Z",
                "updatedAt": "2024-01-01T11:00:00Z",
                "version": 3
              }],
              "deleted": [{"id": "srv-9", "deletedAt": "2024-01-02T00:00:00Z", "version": 2}],
              "serverTime": "2024-01-02T01:00:00Z",
              "nextCursor": null
            }
        """.trimIndent()

        val delta = json.decodeFromString<TracksDeltaDto>(payload)
        assertEquals(1, delta.items.size)
        assertEquals("srv-1", delta.items[0].id)
        assertEquals("Morning loop", delta.items[0].metadata.name)
        assertEquals(2, delta.items[0].geometry.points.size)
        assertEquals(10.5, delta.items[0].geometry.points[0].longitude, 1e-9)
        assertEquals(3L, delta.items[0].version)
        assertEquals(1, delta.deleted.size)
        assertEquals("srv-9", delta.deleted[0].id)
        assertEquals("2024-01-02T01:00:00Z", delta.serverTime)
    }

    @Test
    fun `a remote track maps onto a local entity preserving server identity`() {
        val dto = TrackResponseDto(
            id = "srv-1",
            geometry = TrackGeometryDto(
                points = listOf(WirePoint(10.5, 60.2), WirePoint(10.6, 60.3)),
                elevations = listOf(100.0, 110.0),
            ),
            metadata = TrackMetadataDto(name = "Morning loop"),
            stats = TrackStatsDto(distanceMeters = 2143.0, ascentMeters = 120.0, movingTimeSeconds = 1800, recordedAt = "2024-01-01T12:00:00Z"),
            updatedAt = "2024-01-01T11:00:00Z",
            version = 3,
        )

        val entity = dto.toEntity(localId = "local-1")
        assertEquals("local-1", entity.id)        // local id is the engine's, stable
        assertEquals("srv-1", entity.remoteId)     // server id kept separate
        assertEquals(3L, entity.version)
        assertEquals(false, entity.dirty)          // freshly synced from the server
        assertEquals("Morning loop", entity.name)
        assertEquals(2143.0, entity.distanceM, 1e-9)
        assertEquals("60.2,10.5;60.3,10.6", entity.points) // stored lat,lng
        assertTrue(entity.updatedAtEpochMs!! > 0)
    }

    @Test
    fun `a local entity serializes into a create request with lng-lat points`() {
        val entity = PathEntity(
            id = "local-1",
            name = "Evening walk",
            source = "Recording",
            points = "60.2,10.5;60.3,10.6",
            distanceM = 1500.0,
            ascentM = 50.0,
            descentM = 20.0,
            durationSec = 900,
            createdAtEpochMs = 1_700_000_000_000L,
            elevations = null,
        )

        val req = entity.toWriteRequest()
        assertEquals("Evening walk", req.metadata.name)
        assertEquals(2, req.geometry.points.size)
        assertEquals(10.5, req.geometry.points[0].longitude, 1e-9)
        assertEquals(60.2, req.geometry.points[0].latitude, 1e-9)
        assertEquals(1500.0, req.stats!!.distanceMeters!!, 1e-9)
        assertNull(req.geometry.elevations) // none stored → omitted
    }

    @Test
    fun `display style survives the wire in both directions`() {
        // Pull: a colour/style set on another client lands in the local row…
        val dto = TrackResponseDto(
            id = "srv-2",
            geometry = TrackGeometryDto(points = listOf(WirePoint(10.5, 60.2), WirePoint(10.6, 60.3))),
            metadata = TrackMetadataDto(name = "Styled", colorHex = "#2563EB", iconKey = "hiking", lineStyleKey = "dashed"),
            version = 1,
        )
        val entity = dto.toEntity(localId = "local-2")
        assertEquals("#2563EB", entity.colorHex)
        assertEquals("hiking", entity.iconKey)
        assertEquals("dashed", entity.lineStyleKey)

        // …and push: an Android edit sends it back instead of silently dropping it
        // (the old name-only metadata erased a colour picked on the web).
        val req = entity.toWriteRequest()
        assertEquals("#2563EB", req.metadata.colorHex)
        assertEquals("hiking", req.metadata.iconKey)
        assertEquals("dashed", req.metadata.lineStyleKey)
    }
}
