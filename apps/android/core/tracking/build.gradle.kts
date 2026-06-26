plugins {
    id("turbo.android.library")
    id("turbo.android.hilt")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.tracking"
}

dependencies {
    api(project(":core:model"))
    api(project(":core:common"))
    // FollowController persists/loads tracks through PathRepository. One-way:
    // core:data never depends back on tracking, so the edge stays acyclic.
    implementation(project(":core:data"))

    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.androidx.datastore.preferences)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
    testImplementation(libs.kotlinx.serialization.json) // loads the shared filter fixtures
}
