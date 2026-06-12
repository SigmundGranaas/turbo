plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map"
}

dependencies {
    implementation(project(":core:data"))
    implementation(project(":core:map"))
    implementation(project(":core:turbomap-android"))
    // The home map is the host that composes these sibling features — recording
    // (a map *mode*), the geotagged photo layer, and inline weather/conditions.
    // All three depend only on :core:*, so these edges stay acyclic.
    implementation(project(":feature:recording"))
    implementation(project(":feature:photos"))
    implementation(project(":feature:weather"))

    implementation(libs.androidx.activity.compose)
}
