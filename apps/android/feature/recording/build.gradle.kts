plugins {
    id("turbo.android.feature")
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.recording"
}

dependencies {
    implementation(project(":core:data"))
    implementation(project(":core:sync"))
    implementation(project(":core:map"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.androidx.activity.compose)
}
