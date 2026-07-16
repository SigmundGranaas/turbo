package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Adding a custom map (the user's goal): a working XYZ template is accepted,
 *  anything the engine couldn't substitute tiles into is rejected up front. */
class CustomTileSourceTest {

    @Test
    fun `accepts an https xyz template`() {
        assertTrue(CustomTileSource.isValidTemplate("https://example.com/tiles/{z}/{x}/{y}.png"))
        assertTrue(CustomTileSource.isValidTemplate("  http://tiles.local/{z}/{y}/{x}  "))
        // Query-parameter style templates (Google-like) work too.
        assertTrue(CustomTileSource.isValidTemplate("https://mt1.example.com/vt?x={x}&y={y}&z={z}"))
    }

    @Test
    fun `rejects missing placeholders and bad schemes`() {
        assertFalse(CustomTileSource.isValidTemplate("https://example.com/tiles/{z}/{x}.png"))
        assertFalse(CustomTileSource.isValidTemplate("ftp://example.com/{z}/{x}/{y}.png"))
        assertFalse(CustomTileSource.isValidTemplate("example.com/{z}/{x}/{y}.png"))
        assertFalse(CustomTileSource.isValidTemplate(""))
    }
}
