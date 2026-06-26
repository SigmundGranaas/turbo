plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.offline"
}

dependencies {
    implementation(project(":feature:map-core"))
    implementation(project(":core:data"))
    implementation(project(":core:map"))
}
