import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.artifacts.VersionCatalogsExtension
import org.gradle.kotlin.dsl.getByType

/**
 * The whole stack a feature module needs: Android library + Compose + Hilt, plus
 * the dependencies every feature shares — core model/common/designsystem, the
 * Compose UI cluster, lifecycle + hilt-navigation, and the Compose/Robolectric
 * test cluster. A feature build file then only adds its *specific* core deps
 * (e.g. :core:data, :core:map) and namespace.
 */
class AndroidFeatureConventionPlugin : Plugin<Project> {
    override fun apply(target: Project): Unit = with(target) {
        with(pluginManager) {
            apply("turbo.android.library")
            apply("turbo.android.compose")
            apply("turbo.android.hilt")
        }
        val libs = extensions.getByType<VersionCatalogsExtension>().named("libs")
        fun lib(alias: String) = libs.findLibrary(alias).get()

        dependencies.apply {
            add("implementation", project(":core:model"))
            add("implementation", project(":core:common"))
            add("implementation", project(":core:designsystem"))

            add("implementation", lib("androidx-lifecycle-viewmodel-compose"))
            add("implementation", lib("androidx-lifecycle-runtime-compose"))
            add("implementation", lib("hilt-navigation-compose"))

            add("implementation", lib("androidx-ui"))
            add("implementation", lib("androidx-foundation"))
            add("implementation", lib("androidx-material3"))
            add("implementation", lib("androidx-material-icons-extended"))
            add("implementation", lib("androidx-ui-tooling-preview"))
            add("debugImplementation", lib("androidx-ui-tooling"))

            add("testImplementation", lib("junit"))
            add("testImplementation", lib("kotlinx-coroutines-test"))
            add("testImplementation", lib("turbine"))
            add("testImplementation", lib("robolectric"))
            add("testImplementation", lib("androidx-ui-test-junit4"))
            add("debugImplementation", lib("androidx-ui-test-manifest"))
        }
        Unit
    }
}
