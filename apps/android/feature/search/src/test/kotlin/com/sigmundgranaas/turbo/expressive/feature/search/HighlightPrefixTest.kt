package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.compose.ui.text.font.FontWeight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class HighlightPrefixTest {

    @Test
    fun `bolds the matching prefix only`() {
        val result = highlightPrefix("Storsteinen", "stor")
        assertEquals("Storsteinen", result.text)
        assertEquals(1, result.spanStyles.size)
        val span = result.spanStyles[0]
        assertEquals(0, span.start)
        assertEquals(4, span.end)
        assertEquals(FontWeight.W800, span.item.fontWeight)
    }

    @Test
    fun `non-matching name has no bold span (no StorSjurfjellet artifact)`() {
        val result = highlightPrefix("Sjurfjellet Hytte", "stor")
        assertEquals("Sjurfjellet Hytte", result.text)
        assertTrue(result.spanStyles.isEmpty())
    }

    @Test
    fun `blank query leaves the name plain`() {
        val result = highlightPrefix("Tromsdalstind", "")
        assertEquals("Tromsdalstind", result.text)
        assertTrue(result.spanStyles.isEmpty())
    }
}
