plugins {
    id("turbo.android.library")
    id("turbo.android.compose")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.designsystem"
}

dependencies {
    api(project(":core:model"))

    // Re-exported as `api`: this module is the Compose foundation every UI module
    // builds on, so consumers inherit the Compose surface transitively.
    api(libs.androidx.ui)
    api(libs.androidx.ui.graphics)
    api(libs.androidx.foundation)
    api(libs.androidx.material3)
    api(libs.androidx.material.icons.extended)
    api(libs.androidx.graphics.shapes)
    api(libs.androidx.ui.tooling.preview)
    api(libs.androidx.ui.text.google.fonts)
    debugImplementation(libs.androidx.ui.tooling)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.ui.test.junit4)
    debugImplementation(libs.androidx.ui.test.manifest)
}
