plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.settings"
}

dependencies {
    implementation(project(":core:data"))
    // Real signed-in identity for the account header (replaces the hardcoded block).
    implementation(project(":core:auth"))
}
