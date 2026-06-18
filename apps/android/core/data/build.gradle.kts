plugins {
    id("turbo.android.library")
    id("turbo.android.hilt")
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.data"
    // BuildConfig.DEBUG selects the offline route simulator (see NetworkModule).
    buildFeatures { buildConfig = true }
}

dependencies {
    api(project(":core:model"))
    api(project(":core:common"))

    implementation(libs.kotlinx.coroutines.core)

    implementation(libs.room.runtime)
    implementation(libs.room.ktx)
    ksp(libs.room.compiler)

    implementation(libs.androidx.datastore.preferences)

    implementation(libs.ktor.client.core)
    implementation(libs.ktor.client.okhttp)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
    testImplementation(libs.kotlinx.serialization.json) // loads the shared filter fixtures
}
