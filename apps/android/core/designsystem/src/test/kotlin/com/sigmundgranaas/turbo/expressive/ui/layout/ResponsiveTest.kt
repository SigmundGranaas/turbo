package com.sigmundgranaas.turbo.expressive.ui.layout

import androidx.compose.ui.test.junit4.createComposeRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Compact phone width → Compact class, single column, not expanded. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], qualifiers = "w411dp-h891dp")
class ResponsiveCompactTest {
    @get:Rule val composeRule = createComposeRule()

    @Test
    fun `phone width is Compact`() {
        var cls: WindowWidthClass? = null
        var cols = -1
        var expanded = true
        composeRule.setContent {
            cls = rememberWindowWidthClass()
            cols = adaptiveColumns()
            expanded = isExpandedWidth()
        }
        assertEquals(WindowWidthClass.Compact, cls)
        assertEquals(1, cols)
        assertFalse(expanded)
    }
}

/** Tablet width → Expanded class, two columns, expanded true. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], qualifiers = "w840dp-h1200dp")
class ResponsiveExpandedTest {
    @get:Rule val composeRule = createComposeRule()

    @Test
    fun `tablet width is Expanded`() {
        var cls: WindowWidthClass? = null
        var cols = -1
        var expanded = false
        composeRule.setContent {
            cls = rememberWindowWidthClass()
            cols = adaptiveColumns()
            expanded = isExpandedWidth()
        }
        assertEquals(WindowWidthClass.Expanded, cls)
        assertEquals(2, cols)
        assertTrue(expanded)
    }
}
