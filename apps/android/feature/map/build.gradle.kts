plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map"
}

dependencies {
    implementation(project(":core:data"))
    implementation(project(":core:map"))
    // The home map drives recording directly (recording is a *mode* of the map,
    // not a separate screen). Acyclic: :feature:recording depends only on :core:*.
    implementation(project(":feature:recording"))

    implementation(libs.androidx.activity.compose)
}
