package com.sigmundgranaas.turbo.expressive

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.ext.list.withNameEndingWith
import com.lemonappdev.konsist.api.verify.assertTrue
import org.junit.Test

/**
 * Konsist architecture-boundary guards — these fail the build if the module
 * seams we built start leaking. They scan the whole multi-module project.
 */
class ArchitectureBoundaryTest {

    /**
     * The [com.sigmundgranaas.turbo.expressive.domain.MapEngine] seam is the
     * renderer-agnostic contract feature code talks to — the keystone that keeps
     * the wgpu `TurbomapMapEngine` swappable and a second engine slot-able for
     * shadow parity. If a renderer type ever leaks into the contract itself, the
     * abstraction is dead, so guard it: no GL/renderer SDK import (e.g. the
     * now-removed MapLibre) may appear in the seam.
     */
    @Test
    fun `the MapEngine seam stays renderer-agnostic`() {
        Konsist.scopeFromProject()
            .interfaces()
            .filter { it.name == "MapEngine" }
            .assertTrue { iface ->
                iface.containingFile.imports.none { it.name.startsWith("org.maplibre") }
            }
    }

    @Test
    fun `ViewModels live in feature modules and are named ViewModel`() {
        Konsist.scopeFromProject()
            .classes()
            .withNameEndingWith("ViewModel")
            .assertTrue { it.resideInPackage("..feature..") }
    }

    @Test
    fun `repositories are interfaces declared in a core module`() {
        Konsist.scopeFromProject()
            .interfaces()
            .withNameEndingWith("Repository")
            // Repository interfaces belong in a core module (data, auth, …), never a feature.
            .assertTrue { it.resideInPackage("..core..") }
    }

    /**
     * Feature modules don't depend on each other — except :feature:map, the home
     * screen that *composes* a fixed set of sub-features. This pins that one
     * allowed edge so a second feature→feature dependency can't sneak in.
     */
    @Test
    fun `only the map host may depend on other feature modules`() {
        // The primary code package of each feature *module* (weather's is `conditions`;
        // the map host's remaining sub-packages — layers/nav — are not modules, so they're
        // deliberately absent. The extracted map tools (map-*) are skipped above and
        // governed by the dedicated map-tier rules.
        val modulePackages = setOf(
            "map", "recording", "photos", "conditions",
            "auth", "search", "settings", "collections",
        )
        val mapComposes = setOf("recording", "photos", "conditions")
        val importedFeature = Regex("""com\.sigmundgranaas\.turbo\.expressive\.feature\.([a-z]+)""")
        val owningDir = Regex("""/feature/([^/]+)/""")

        Konsist.scopeFromProject()
            .files
            .filter { it.path.contains("/feature/") }
            .assertTrue { file ->
                val dir = owningDir.find(file.path)?.groupValues?.get(1) ?: return@assertTrue true
                // The map tier (map-core + map-<tool>) has its own dedicated rules below.
                if (dir.startsWith("map-")) return@assertTrue true
                val ownPackage = if (dir == "weather") "conditions" else dir
                file.imports.all { import ->
                    val target = importedFeature.find(import.name)?.groupValues?.get(1)
                    when {
                        target == null || target !in modulePackages -> true // not a cross-module edge
                        target == ownPackage -> true                        // within the same module
                        dir == "map" -> target in mapComposes               // the host's allowed edges
                        else -> false                                       // any other edge is banned
                    }
                }
            }
    }

    // ── The map tier (see docs/architecture/2026-06-android-architecture-remediation-plan.md) ──
    // The home map is split into a passive kernel (:feature:map-core) + leaf tool
    // modules (:feature:map-sun/-radar/-live/-markers/-offline/-collectionpicker/-route).
    // Each tool dir and its primary package:
    private val mapToolDirs = mapOf(
        "map-sun" to "feature.map.sun",
        "map-radar" to "feature.map.radar",
        "map-live" to "feature.map.live",
        "map-route" to "feature.map.route",
        "map-markers" to "feature.markers",
        "map-offline" to "feature.offline",
        "map-collectionpicker" to "feature.collectionpicker",
    )

    /**
     * Map tool modules are *leaves*: a tool may import the kernel (`feature.map.core`)
     * and `core:*`, but never another tool and never the host (`feature.map.*` types).
     * Cross-tool behaviour must flow through a `core:*` seam (e.g. `FollowController`).
     */
    @Test
    fun `map tool modules are leaves`() {
        val allToolPkgs = mapToolDirs.values.toSet()
        // A host type import looks like `...feature.map.<Capitalised>` — sub-packages
        // (feature.map.core / .sun / .radar / .live / .route) are lower-case, so excluded.
        val hostType = Regex("""\.feature\.map\.[A-Z]""")
        Konsist.scopeFromProject()
            .files
            .filter { f -> mapToolDirs.keys.any { f.path.contains("/feature/$it/") } }
            .assertTrue { file ->
                val ownDir = mapToolDirs.keys.first { file.path.contains("/feature/$it/") }
                val ownPkg = mapToolDirs.getValue(ownDir)
                file.imports.none { import ->
                    val n = import.name
                    val otherTool = allToolPkgs.any { it != ownPkg && n.contains("$it.") }
                    otherTool || hostType.containsMatchIn(n)
                }
            }
    }

    /** The kernel is passive: it depends on `core:*` only, never on the feature tier. */
    @Test
    fun `the map kernel imports nothing from the feature tier`() {
        Konsist.scopeFromProject()
            .files
            .filter { it.path.contains("/feature/map-core/") }
            .assertTrue { file -> file.imports.none { it.name.contains(".expressive.feature.") } }
    }
}
