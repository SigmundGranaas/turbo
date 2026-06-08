plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.collections"
}

dependencies {
    implementation(project(":core:data"))
}
