plugins {
    id("turbo.android.feature")
}

android {
    // Package kept as `feature.conditions` (the code's package) so no imports churn.
    namespace = "com.sigmundgranaas.turbo.expressive.feature.conditions"
}

dependencies {
    implementation(project(":core:data"))
}
