plugins {
    id("turbo.android.library")
    id("turbo.android.compose")
    id("turbo.android.hilt")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.map"
}

dependencies {
    api(project(":core:model"))
    api(project(":core:designsystem"))
    // The non-MapLibre offline manager pre-populates the wgpu map's tile store
    // (TileStore / TURBOMAP_TILE_DIR) so downloaded regions render with no network.
    implementation(project(":core:turbomap-android"))

    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.okhttp)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.ui.test.junit4)
    testImplementation(libs.kotlinx.coroutines.test)
}
