import org.gradle.api.Project
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

internal const val TURBO_COMPILE_SDK = 37
internal const val TURBO_MIN_SDK = 26

/** Pin every Kotlin compilation in the project to JVM 17 (matches compileOptions). */
internal fun Project.configureKotlinJvm() {
    tasks.withType(KotlinCompile::class.java).configureEach {
        compilerOptions.jvmTarget.set(JvmTarget.JVM_17)
    }
}
