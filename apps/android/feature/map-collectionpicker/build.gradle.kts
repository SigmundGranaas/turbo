plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.collectionpicker"
}

dependencies {
    implementation(project(":feature:map-core"))
    implementation(project(":core:data"))
}
