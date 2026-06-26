plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.markers"
}

dependencies {
    implementation(project(":feature:map-core"))
}
