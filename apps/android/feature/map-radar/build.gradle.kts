plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map.radar"
}

dependencies {
    implementation(project(":feature:map-core"))
    implementation(project(":core:data"))
}
