plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map"
}

dependencies {
    implementation(project(":core:data"))
    implementation(project(":core:tracking"))
    implementation(project(":core:map"))
    implementation(project(":core:turbomap-android"))
    // The map tier: the passive kernel + the leaf tool modules the host composes.
    // Each map-* tool depends only on map-core + core:*, never on each other.
    implementation(project(":feature:map-core"))
    implementation(project(":feature:map-sun"))
    implementation(project(":feature:map-radar"))
    implementation(project(":feature:map-live"))
    implementation(project(":feature:map-markers"))
    implementation(project(":feature:map-collectionpicker"))
    implementation(project(":feature:map-offline"))
    implementation(project(":feature:map-route"))
    // The home map is the host that composes these sibling features — recording
    // (a map *mode*), the geotagged photo layer, and inline weather/conditions.
    // All three depend only on :core:*, so these edges stay acyclic.
    implementation(project(":feature:recording"))
    implementation(project(":feature:photos"))
    implementation(project(":feature:weather"))

    implementation(libs.androidx.activity.compose)
}
