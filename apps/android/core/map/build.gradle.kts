plugins {
    id("turbo.android.library")
    id("turbo.android.compose")
    id("turbo.android.hilt")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.map"
    // BuildConfig.DEBUG selects the offline tile-manager simulator.
    buildFeatures { buildConfig = true }
}

dependencies {
    api(project(":core:model"))
    api(project(":core:designsystem"))

    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.maplibre)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.ui.test.junit4)
}
