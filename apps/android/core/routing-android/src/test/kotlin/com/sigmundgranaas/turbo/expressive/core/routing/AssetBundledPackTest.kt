package com.sigmundgranaas.turbo.expressive.core.routing

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayInputStream
import java.io.File

/**
 * Installing the APK's own pack.
 *
 * The point of the bundled pack is that once installed it is
 * *indistinguishable* from a downloaded one — same key, same manifest,
 * found by the same `PackStore` scan. So the assertions here are about
 * what `PackStore` can then see, not about files having been copied.
 */
class AssetBundledPackTest {

    @get:Rule
    val tmp = TemporaryFolder()

    /** Just enough of an AssetManager to serve one pack directory. */
    private class FakeAssets(private val files: Map<String, ByteArray>) : PackAssets {
        override fun list(path: String): List<String> =
            files.keys.filter { it.startsWith("$path/") }.map { it.removePrefix("$path/") }

        override fun open(path: String) =
            ByteArrayInputStream(files[path] ?: error("no asset $path"))
    }

    private val key = "z12_2219_1001_2232_1014"

    private fun assets(
        manifest: String = "extent = [15.029297, 66.826520, 16.259766, 67.305976]\n",
    ) = FakeAssets(
        mapOf(
            "${AssetBundledPack.ASSET_ROOT}/$key/norway.dem" to ByteArray(64) { 1 },
            "${AssetBundledPack.ASSET_ROOT}/$key/norway.mask" to ByteArray(32) { 2 },
            "${AssetBundledPack.ASSET_ROOT}/$key/norway.graph" to ByteArray(16) { 3 },
            "${AssetBundledPack.ASSET_ROOT}/$key/norway.graph_geom" to ByteArray(16) { 4 },
            "${AssetBundledPack.ASSET_ROOT}/$key/pack.toml" to manifest.toByteArray(),
        ),
    )

    private fun pack(root: File) = AssetBundledPack(assets(), root, key = key)

    @Test
    fun `an installed pack is one PackStore will route with`() = runTest {
        val root = tmp.newFolder("routing-packs")
        val p = pack(root)
        assertFalse(p.isInstalled())

        assertTrue(p.install().isSuccess)
        assertTrue(p.isInstalled())

        // The real assertion: the pack store finds it and answers a
        // coverage question with it. A test that only checked for files
        // on disk would pass with a manifest PackStore cannot parse.
        val store = PackStore(root)
        val found = store.covering(listOf(15.5 to 67.0))
        assertEquals(key, found?.id)
        assertEquals(4 + 1, found!!.dir.listFiles()!!.size)
    }

    @Test
    fun `a pack outside the region is not claimed`() = runTest {
        val root = tmp.newFolder("routing-packs")
        pack(root).install()
        // Oslo — nowhere near Sjunkhatten. Claiming it would send the
        // solver at terrain the pack does not contain.
        assertEquals(null, PackStore(root).covering(listOf(10.75 to 59.91)))
    }

    @Test
    fun `installing twice is a no-op, not a corruption`() = runTest {
        val root = tmp.newFolder("routing-packs")
        val p = pack(root)
        p.install()
        val before = File(root, key).listFiles()!!.map { it.name to it.length() }.sortedBy { it.first }
        assertTrue(p.install().isSuccess)
        val after = File(root, key).listFiles()!!.map { it.name to it.length() }.sortedBy { it.first }
        assertEquals(before, after)
    }

    @Test
    fun `a failed install leaves nothing behind`() = runTest {
        // A pack without its manifest is not a pack. Half-copying it
        // into place would give PackStore a directory it skips and the
        // user a silent "install" that did nothing — so the staging
        // directory has to go too.
        val root = tmp.newFolder("routing-packs")
        val broken = AssetBundledPack(
            assets = FakeAssets(
                mapOf("${AssetBundledPack.ASSET_ROOT}/$key/norway.dem" to ByteArray(8)),
            ),
            root = root,
            key = key,
        )
        assertTrue(broken.install().isFailure)
        assertFalse(broken.isInstalled())
        assertEquals(
            "no leftovers: ${root.listFiles()?.map { it.name }}",
            0,
            root.listFiles()!!.size,
        )
    }

    @Test
    fun `uninstalling frees the region again`() = runTest {
        val root = tmp.newFolder("routing-packs")
        val p = pack(root)
        p.install()
        p.uninstall()
        assertFalse(p.isInstalled())
        assertEquals(null, PackStore(root).covering(listOf(15.5 to 67.0)))
    }
}
