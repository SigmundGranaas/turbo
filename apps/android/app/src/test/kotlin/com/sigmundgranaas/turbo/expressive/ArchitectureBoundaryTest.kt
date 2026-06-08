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

    @Test
    fun `MapLibre is referenced only inside core map`() {
        Konsist.scopeFromProject()
            .files
            .filter { file -> file.imports.any { it.name.startsWith("org.maplibre") } }
            .assertTrue { it.path.contains("/core/map/") }
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
        // map's own sub-packages — layers/markers/nav/offline/collectionpicker — are not
        // modules, so they're deliberately absent and never flagged).
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
}
