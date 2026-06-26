plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map.sun"
}

dependencies {
    implementation(project(":feature:map-core"))
}
