plugins {
    id("turbo.android.library")
    id("turbo.android.hilt")
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.auth"
}

dependencies {
    api(project(":core:model"))
    api(project(":core:common"))

    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.androidx.datastore.preferences)

    implementation(libs.ktor.client.core)
    implementation(libs.ktor.client.okhttp)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.ktor.client.mock)
    testImplementation(libs.ktor.client.content.negotiation)
    testImplementation(libs.ktor.serialization.kotlinx.json)
}
