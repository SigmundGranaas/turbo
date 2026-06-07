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
}
