plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map.route"
}

dependencies {
    implementation(project(":feature:map-core"))
    implementation(project(":core:data"))
    implementation(project(":core:map"))
    implementation(project(":core:tracking"))
}
