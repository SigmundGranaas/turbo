plugins {
    id("turbo.android.feature")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.auth"
}

dependencies {
    implementation(project(":core:auth"))
    implementation(project(":core:sync"))
    // Chrome Custom Tab for the Google consent page (in-app browser surface).
    implementation(libs.androidx.browser)
}
