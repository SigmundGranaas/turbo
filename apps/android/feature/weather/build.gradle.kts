plugins {
    id("turbo.android.feature")
}

android {
    // Package kept as `feature.conditions` (the code's package) so no imports churn.
    namespace = "com.sigmundgranaas.turbo.expressive.feature.conditions"
}

dependencies {
    implementation(project(":core:data"))
    // yr.no weather symbols are SVG assets (matching the Flutter app); Coil renders them.
    implementation(libs.coil.compose)
    implementation(libs.coil.svg)
}
