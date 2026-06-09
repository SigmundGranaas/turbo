package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.TideKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TideXmlTest {

    @Test
    fun `parses extrema, maps flags, reads station, sorts by time`() {
        val xml = """
            <tide>
              <locationdata>
                <location name="Tromsø" code="TOS" latitude="69.65" longitude="18.96">
                  <data type="prediction" unit="cm" qualityflag="0">
                    <waterlevel value="14.0" time="2026-06-09T09:12:00+02:00" flag="low"/>
                    <waterlevel value="121.5" time="2026-06-09T03:00:00+02:00" flag="high"/>
                  </data>
                </location>
              </locationdata>
            </tide>
        """.trimIndent()

        val forecast = TideXml.parse(xml)!!
        assertEquals("Tromsø", forecast.stationName)
        assertEquals(2, forecast.extrema.size)
        // Sorted ascending by time → the 03:00 high comes first.
        assertEquals(TideKind.High, forecast.extrema[0].kind)
        assertEquals(121.5, forecast.extrema[0].levelCm, 1e-9)
        assertEquals(3, forecast.extrema[0].hour)
        assertEquals(TideKind.Low, forecast.extrema[1].kind)
    }

    @Test
    fun `no waterlevel rows yields null (off the coast)`() {
        assertNull(TideXml.parse("<tide><locationdata><location name=\"X\"/></locationdata></tide>"))
        assertNull(TideXml.parse(""))
    }

    @Test
    fun `nextAfter finds the first extremum after a time`() {
        val xml = """
            <tide><locationdata><location name="X">
            <waterlevel value="100" time="2026-06-09T03:00:00+02:00" flag="high"/>
            <waterlevel value="10" time="2026-06-09T09:00:00+02:00" flag="low"/>
            </location></locationdata></tide>
        """.trimIndent()
        val f = TideXml.parse(xml)!!
        assertEquals(TideKind.Low, f.nextAfter("2026-06-09T05:00:00+02:00")?.kind)
    }
}
