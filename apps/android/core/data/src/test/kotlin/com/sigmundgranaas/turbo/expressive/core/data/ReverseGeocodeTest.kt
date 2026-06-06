package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReverseGeocodeTest {

    @Test
    fun `nearest name prefers a close peak over a far settlement`() {
        val candidates = listOf(
            ReverseGeocode.NearbyName("Lom", "tettsted", 1800.0),
            ReverseGeocode.NearbyName("Galdhøpiggen", "fjelltopp", 80.0),
        )
        val best = ReverseGeocode.pickNearestName(candidates)
        assertEquals("Galdhøpiggen", best!!.name)
        assertEquals(PlaceQualifier.On, ReverseGeocode.qualifierFor(best))
    }

    @Test
    fun `a distant peak beyond its radius is dropped in favour of a settlement`() {
        val candidates = listOf(
            ReverseGeocode.NearbyName("Lom", "tettsted", 1200.0),
            ReverseGeocode.NearbyName("Faraway topp", "fjelltopp", 5000.0),
        )
        val best = ReverseGeocode.pickNearestName(candidates)
        assertEquals("Lom", best!!.name)
        assertEquals(PlaceQualifier.In, ReverseGeocode.qualifierFor(best))
    }

    @Test
    fun `stedsnavn json parses name, type and distance`() {
        val body = """
            {"navn":[
              {"skrivemåte":"Besseggen","navneobjekttype":"rygg","meterFraPunkt":42.0}
            ]}
        """.trimIndent()
        val hits = ReverseGeocode.parseStedsnavn(body)
        assertEquals(1, hits.size)
        assertEquals("Besseggen", hits[0].name)
        assertEquals(42.0, hits[0].distanceM, 1e-9)
    }

    @Test
    fun `elevation is parsed and out-of-range rejected`() {
        assertEquals(2469.0, ReverseGeocode.parseElevation("""{"punkter":[{"z":2469.0}]}""")!!, 1e-6)
        assertNull(ReverseGeocode.parseElevation("""{"punkter":[{"z":99999.0}]}"""))
        assertNull(ReverseGeocode.parseElevation("garbage"))
    }

    @Test
    fun `kommune and address parse`() {
        val k = ReverseGeocode.parseKommune("""{"kommunenavn":"Lom","fylkesnavn":"Innlandet"}""")
        assertEquals("Lom", k!!.name)
        assertEquals("Innlandet", k.fylke)
        assertEquals("Storgata 4", ReverseGeocode.parseAddress("""{"adresser":[{"adressetekst":"Storgata 4"}]}"""))
        assertNull(ReverseGeocode.parseAddress("""{"adresser":[]}"""))
    }

    @Test
    fun `compose cascades name then address then kommune`() {
        val peak = ReverseGeocode.NearbyName("Galdhøpiggen", "fjelltopp", 80.0)
        val byName = ReverseGeocode.compose(peak, "ignored", ReverseGeocode.Kommune("Lom", "Innlandet"), 2469.0)!!
        assertEquals("On Galdhøpiggen", byName.label)
        assertTrue(byName.subtitle.contains("2469 m"))

        val byAddress = ReverseGeocode.compose(null, "Storgata 4", ReverseGeocode.Kommune("Lom", null), null)!!
        assertEquals("Near Storgata 4", byAddress.label)

        val byKommune = ReverseGeocode.compose(null, null, ReverseGeocode.Kommune("Lom", "Innlandet"), null)!!
        assertEquals("In Lom", byKommune.label)

        assertNull(ReverseGeocode.compose(null, null, null, null))
    }

    @Test
    fun `cell key is stable within a cell and differs across cells`() {
        val a = ReverseGeocode.cellKey(LatLng(61.6361, 8.3128))
        val aNudged = ReverseGeocode.cellKey(LatLng(61.6362, 8.3129))
        val far = ReverseGeocode.cellKey(LatLng(61.7000, 8.4000))
        assertEquals(a, aNudged)
        assertNotEquals(a, far)
    }
}
