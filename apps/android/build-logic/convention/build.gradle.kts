plugins {
    `kotlin-dsl`
}

group = "com.sigmundgranaas.turbo.buildlogic"

dependencies {
    // compileOnly: the real plugin jars are provided by the consuming build's
    // classpath at apply time; we only need them to compile against here.
    compileOnly(libs.android.gradlePlugin)
    compileOnly(libs.kotlin.gradlePlugin)
    compileOnly(libs.compose.gradlePlugin)
    compileOnly(libs.ksp.gradlePlugin)
    compileOnly(libs.hilt.gradlePlugin)
}

gradlePlugin {
    plugins {
        register("androidLibrary") {
            id = "turbo.android.library"
            implementationClass = "AndroidLibraryConventionPlugin"
        }
        register("androidApplication") {
            id = "turbo.android.application"
            implementationClass = "AndroidApplicationConventionPlugin"
        }
        register("androidCompose") {
            id = "turbo.android.compose"
            implementationClass = "AndroidComposeConventionPlugin"
        }
        register("hilt") {
            id = "turbo.android.hilt"
            implementationClass = "HiltConventionPlugin"
        }
        register("androidFeature") {
            id = "turbo.android.feature"
            implementationClass = "AndroidFeatureConventionPlugin"
        }
    }
}
