plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.settings"
}

dependencies {
    implementation(project(":core:data"))
}
