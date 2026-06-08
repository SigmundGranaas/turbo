import com.android.build.api.dsl.LibraryExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure

/**
 * Turns on Compose for an Android *library* module: applies the Compose compiler
 * plugin and the `compose` build feature, and lets unit tests see Android
 * resources (Robolectric). Does NOT add Compose dependencies — :core:designsystem
 * re-exports those as `api`, and each feature pulls them via [turbo.android.feature].
 */
class AndroidComposeConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) = with(target) {
        pluginManager.apply("org.jetbrains.kotlin.plugin.compose")
        // Defer until the Android library plugin is present, regardless of order.
        pluginManager.withPlugin("com.android.library") {
            extensions.configure<LibraryExtension> {
                buildFeatures { compose = true }
                testOptions { unitTests { isIncludeAndroidResources = true } }
            }
        }
    }
}
