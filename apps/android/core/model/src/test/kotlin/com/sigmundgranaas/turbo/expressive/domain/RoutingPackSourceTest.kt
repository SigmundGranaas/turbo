package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [RoutingPack.urlFor] — the one piece of the pack-source setting that
 * can be wrong without failing loudly.
 *
 * A URL built wrong does not throw; it 404s, and a 404 on the manifest
 * is [PackDownloader.Outcome.Unsupported], which the app deliberately
 * treats as "this host does not serve packs" and completes the region
 * without one. So the failure mode of a bad template is a download that
 * silently succeeds with no routing — exactly the thing these cases
 * exist to keep out.
 */
class RoutingPackSourceTest {

    private val key = "z12_2219_1001_2232_1014"

    @Test
    fun `a plain base url keeps the directory layout the tileserver serves`() {
        assertEquals(
            "https://kart-api.sandring.no/v1/packs/$key/norway.dem",
            RoutingPack.urlFor("https://kart-api.sandring.no/v1/packs", key, "norway.dem"),
        )
    }

    @Test
    fun `a trailing slash does not double up`() {
        assertEquals(
            "https://example.test/packs/$key/pack.toml",
            RoutingPack.urlFor("https://example.test/packs/", key, "pack.toml"),
        )
    }

    /** Release assets are one flat namespace per tag, so the key moves into the name. */
    @Test
    fun `placeholders put the key in the file name`() {
        assertEquals(
            "https://example.test/download/tag/$key-norway.dem",
            RoutingPack.urlFor("https://example.test/download/tag/{key}-{file}", key, "norway.dem"),
        )
    }

    @Test
    fun `either placeholder alone is enough to opt out of the directory form`() {
        assertEquals(
            "https://example.test/norway.dem",
            RoutingPack.urlFor("https://example.test/{file}", key, "norway.dem"),
        )
    }

    /**
     * The shipped default has to be a template, not a base URL — a
     * release asset cannot live in a directory. If someone edits it back
     * into a plain base URL every download 404s, and per the class
     * comment that failure is silent.
     */
    @Test
    fun `the default source resolves to a flat release asset`() {
        val url = RoutingPack.urlFor(RoutingPack.DEFAULT_SOURCE, key, RoutingPack.MANIFEST)
        assertTrue("default must not contain unexpanded tokens: $url", !url.contains("{"))
        assertTrue("default must be a flat asset name: $url", url.endsWith("/$key-${RoutingPack.MANIFEST}"))
    }
}
