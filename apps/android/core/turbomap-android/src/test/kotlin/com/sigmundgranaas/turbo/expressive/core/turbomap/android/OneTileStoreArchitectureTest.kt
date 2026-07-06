package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Architecture tripwire (plan P6.6): the device has ONE tile store.
 *
 * The architecture assigns tile IO to the host ("auth, caching, offline" —
 * `turbomap-engine/src/host_resolver.rs`), and the host's design is one
 * shared [TileStore]: the map's fetch loop reads through it, the offline
 * downloader pre-populates it. A second persistence layer (an OkHttp disk
 * `.cache(`) would silently double-store every tile and detach the offline
 * manager's size accounting from reality — this scan makes that a test
 * failure instead of a code-review catch.
 *
 * Same ratchet rule as the Rust `invariants.rs` gate: the source scan may
 * only get stricter.
 */
class OneTileStoreArchitectureTest {

    @Test
    fun `no OkHttp disk cache exists beside the one TileStore`() {
        val offenders = mapMainSources()
            .filter { file ->
                file.readText().lines().any { line ->
                    val code = line.substringBefore("//")
                    code.contains(".cache(") && code.contains("okhttp", ignoreCase = true) ||
                        code.contains("Cache(File(")
                }
            }
        assertTrue(
            "OkHttp disk caches beside TileStore (one-store law, plan P6.6): " +
                offenders.joinToString { it.path },
            offenders.isEmpty(),
        )
    }

    @Test
    fun `the fetch loop reads through the shared store before the network`() {
        // Tripwire, not proof: the read-through line must exist in the host's
        // fetch loop. The behavioural gate is the on-device airplane test
        // (TurbomapOfflineOnDeviceTest).
        val view = mapMainSources().first { it.name == "TurbomapMapView.kt" }.readText()
        assertTrue(
            "TurbomapMapView must consult the TileStore before fetching",
            view.contains("tileCache?.get("),
        )
        val downloader = mapMainSources().first { it.name == "WgpuOfflineTileManager.kt" }.readText()
        assertTrue(
            "the offline downloader must write the SAME shared store dir",
            downloader.contains("TURBOMAP_TILE_DIR"),
        )
    }

    /** Main-source Kotlin files of the two modules that touch tiles on disk:
     *  this one (turbomap-android) and core/map (the offline manager). */
    private fun mapMainSources(): List<File> {
        val root = generateSequence(File(".").canonicalFile) { it.parentFile }
            .first { File(it, "settings.gradle.kts").isFile || File(it, "settings.gradle").isFile }
        return listOf(
            File(root, "core/turbomap-android/src/main"),
            File(root, "core/map/src/main"),
        ).flatMap { dir ->
            dir.walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()
        }
    }
}
