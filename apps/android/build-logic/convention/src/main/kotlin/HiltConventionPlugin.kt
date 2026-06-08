import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.artifacts.VersionCatalogsExtension
import org.gradle.kotlin.dsl.getByType

/** Applies KSP + Hilt and wires the Hilt runtime + compiler. */
class HiltConventionPlugin : Plugin<Project> {
    override fun apply(target: Project): Unit = with(target) {
        with(pluginManager) {
            apply("com.google.devtools.ksp")
            apply("com.google.dagger.hilt.android")
        }
        val libs = extensions.getByType<VersionCatalogsExtension>().named("libs")
        dependencies.add("implementation", libs.findLibrary("hilt-android").get())
        dependencies.add("ksp", libs.findLibrary("hilt-compiler").get())
        Unit
    }
}
