package com.sigmundgranaas.turbo.expressive.feature.photos

import com.sigmundgranaas.turbo.expressive.domain.Photo
import org.junit.Assert.assertEquals
import org.junit.Test

class PhotoClusteringTest {

    private fun photo(id: String, lat: Double, lng: Double, t: Long = 0) =
        Photo(id = id, markerId = null, lat = lat, lng = lng, uri = "/$id.jpg", capturedAtEpochMs = t)

    @Test
    fun `nearby photos collapse into one cluster`() {
        val photos = listOf(
            photo("a", 69.6000, 18.9000),
            photo("b", 69.60002, 18.90003), // a few metres away
            photo("c", 69.60001, 18.90001),
        )
        val clusters = clusterPhotos(photos, gridDeg = 0.0008)
        assertEquals(1, clusters.size)
        assertEquals(3, clusters.first().count)
    }

    @Test
    fun `far-apart photos stay separate`() {
        val clusters = clusterPhotos(
            listOf(photo("a", 69.60, 18.90), photo("b", 60.10, 9.20)),
            gridDeg = 0.0008,
        )
        assertEquals(2, clusters.size)
    }

    @Test
    fun `cover is the newest photo and order is newest-first`() {
        val cluster = clusterPhotos(
            listOf(photo("old", 69.6, 18.9, t = 100), photo("new", 69.6, 18.9, t = 500)),
        ).single()
        assertEquals("/new.jpg", cluster.coverUri)
        assertEquals(listOf("new", "old"), cluster.ordered.map { it.id })
    }

    @Test
    fun `empty input yields no clusters`() {
        assertEquals(emptyList<PhotoCluster>(), clusterPhotos(emptyList()))
    }
}
