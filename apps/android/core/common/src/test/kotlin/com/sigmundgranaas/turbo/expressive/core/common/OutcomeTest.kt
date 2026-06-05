package com.sigmundgranaas.turbo.expressive.core.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OutcomeTest {

    @Test
    fun `map transforms a success value`() {
        val result = Outcome.Success(2).map { it * 10 }
        assertEquals(20, (result as Outcome.Success).value)
    }

    @Test
    fun `map leaves a failure untouched and does not run the transform`() {
        val boom = IllegalStateException("boom")
        var ran = false
        val result = (Outcome.Failure(boom) as Outcome<Int>).map { ran = true; it }
        assertEquals(boom, (result as Outcome.Failure).error)
        assertEquals(false, ran)
    }

    @Test
    fun `getOrNull returns value or null`() {
        assertEquals(7, Outcome.Success(7).getOrNull())
        assertNull(Outcome.Failure(RuntimeException()).getOrNull())
    }

    @Test
    fun `catching wraps a throw as failure`() {
        val result = Outcome.catching<Int> { error("nope") }
        assert(result is Outcome.Failure)
    }
}
