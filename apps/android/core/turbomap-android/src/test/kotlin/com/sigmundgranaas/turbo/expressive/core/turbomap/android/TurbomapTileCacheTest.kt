package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** The read-through disk cache the turbomap host uses for raster tiles (pure JVM File IO). */
class TurbomapTileCacheTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private fun cache() = TurbomapTileCache(tmp.newFolder("tiles"))

    @Test
    fun `miss returns null`() {
        assertNull(cache().get("basemap", 9, 1, 2))
    }

    @Test
    fun `put then get round-trips the bytes`() {
        val cache = cache()
        val bytes = byteArrayOf(1, 2, 3, 4, 5)
        cache.put("basemap", 9, 273, 132, bytes)
        assertArrayEquals(bytes, cache.get("basemap", 9, 273, 132))
    }

    @Test
    fun `distinct tiles do not collide`() {
        val cache = cache()
        cache.put("basemap", 9, 1, 1, byteArrayOf(1))
        cache.put("basemap", 9, 1, 2, byteArrayOf(2))
        cache.put("ov_Trails", 9, 1, 1, byteArrayOf(3))
        assertArrayEquals(byteArrayOf(1), cache.get("basemap", 9, 1, 1))
        assertArrayEquals(byteArrayOf(2), cache.get("basemap", 9, 1, 2))
        assertArrayEquals(byteArrayOf(3), cache.get("ov_Trails", 9, 1, 1))
    }

    @Test
    fun `empty bytes are not stored`() {
        val cache = cache()
        cache.put("basemap", 9, 1, 1, ByteArray(0))
        assertNull(cache.get("basemap", 9, 1, 1))
    }

    @Test
    fun `layer ids with path-unsafe characters are sanitised, not crashing`() {
        val cache = cache()
        val bytes = byteArrayOf(7, 7)
        cache.put("a/b\\c:d", 3, 0, 0, bytes)
        assertArrayEquals(bytes, cache.get("a/b\\c:d", 3, 0, 0))
        assertTrue(true) // reached here = no path traversal / IO crash
    }
}
