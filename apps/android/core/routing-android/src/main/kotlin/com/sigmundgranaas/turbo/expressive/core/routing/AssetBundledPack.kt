package com.sigmundgranaas.turbo.expressive.core.routing

import android.content.res.AssetManager
import com.sigmundgranaas.turbo.expressive.core.data.BundledRoutingPack
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.InputStream

/**
 * The two things this needs from the APK's assets.
 *
 * An interface rather than `AssetManager` because that class is final
 * and package-private to construct, so depending on it directly would
 * put this class's only interesting logic — staging, ordering, cleanup
 * on failure — behind an emulator.
 */
interface PackAssets {
    fun list(path: String): List<String>
    fun open(path: String): InputStream

    companion object {
        fun of(assets: AssetManager) = object : PackAssets {
            override fun list(path: String) = assets.list(path)?.toList().orEmpty()
            override fun open(path: String): InputStream = assets.open(path)
        }
    }
}

/**
 * The bundled pack, read out of the APK's `assets/bundled-pack/<key>/`.
 *
 * Cut from the real Sjunkhatten artifacts with `tileserver slice-pack`,
 * so it is genuine terrain and a genuine trail graph — 9 712 directed
 * edges over 53 x 53 km, not a fixture. A synthetic pack would answer a
 * different question: the solver's cost is a function of the graph it
 * walks, and a toy graph would make the phone look fast for a reason
 * that does not generalise.
 *
 * The region is where the artifacts are, not where any particular user
 * is. That is fine — planning a route does not require standing in it,
 * and what is being measured is the engine, not the scenery.
 */
class AssetBundledPack(
    private val assets: PackAssets,
    /** The pack store's root — `filesDir/routing-packs`. */
    private val root: File,
    override val key: String = KEY,
    override val sizeBytes: Long = SIZE_BYTES,
    override val description: String = DESCRIPTION,
) : BundledRoutingPack {

    private val dir get() = File(root, key)

    /**
     * Judged by the manifest, which [install] writes into place last —
     * so a directory with one is a directory that finished copying.
     */
    override fun isInstalled(): Boolean = File(dir, MANIFEST).isFile

    override suspend fun install(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            if (isInstalled()) return@runCatching
            val staging = File(root, "$key.partial")
            staging.deleteRecursively()
            if (!staging.mkdirs()) error("Couldn't create $staging")

            val assetDir = "$ASSET_ROOT/$key"
            val names = assets.list(assetDir)
            check(names.isNotEmpty()) { "no bundled pack at assets/$assetDir" }
            check(MANIFEST in names) { "bundled pack has no $MANIFEST" }

            // The manifest last, for the same reason `install` is atomic
            // at all: it is what marks the pack complete, and writing it
            // before the DEM would let an interrupted copy present a
            // truncated pack as a finished one.
            for (name in names.filter { it != MANIFEST } + MANIFEST) {
                assets.open("$assetDir/$name").use { input ->
                    File(staging, name).outputStream().use { input.copyTo(it) }
                }
            }

            dir.deleteRecursively()
            check(staging.renameTo(dir)) { "couldn't move $staging into place" }
        }.onFailure {
            File(root, "$key.partial").deleteRecursively()
        }
    }

    override suspend fun uninstall(): Unit = withContext(Dispatchers.IO) {
        dir.deleteRecursively()
        File(root, "$key.partial").deleteRecursively()
    }

    companion object {
        const val ASSET_ROOT = "bundled-pack"
        const val MANIFEST = "pack.toml"

        /**
         * Grid-aligned so it looks exactly like a downloaded pack —
         * same key format, same directory name, same manifest. Nothing
         * downstream can tell the difference, which is the point: the
         * measurement has to exercise the real path.
         */
        const val KEY = "z12_2219_1001_2232_1014"

        /** 53 x 53 km over Sjunkhatten; 4 293 nodes, 9 712 directed edges. */
        const val DESCRIPTION = "Sjunkhatten · 53 × 53 km"

        /** Measured after slicing. Shown before the user spends the disk. */
        const val SIZE_BYTES = 55_075_080L
    }
}
