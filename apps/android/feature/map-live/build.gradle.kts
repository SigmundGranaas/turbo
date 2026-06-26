plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map.live"
}

dependencies {
    implementation(project(":feature:map-core"))
    implementation(project(":core:tracking"))
}
