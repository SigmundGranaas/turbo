import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

/**
 * `:core:routing-android` — the on-device routing engine, packaged for Android.
 *
 * Deliberately a near-copy of `:core:turbomap-android`'s Rust wiring, because
 * that module already paid for the mistakes: bindings are generated from the
 * HOST cdylib (uniffi-bindgen loads the library to read its metadata, so it must
 * match the build machine's arch), the per-ABI `.so` comes from cargo-ndk, and
 * JNA arrives as the **@aar** whose own `libjnidispatch.so` is built for Android.
 * Mixing the desktop JNA jar and the Android aar in one module is the classic
 * uniffi-on-Android footgun; two modules, two artifacts, no mixing.
 *
 * Two things differ from that module, both deliberate.
 *
 * The host cdylib is built **debug**, not release: `[profile.release]` in the
 * tileserver workspace sets `strip = "symbols"`, and library-mode uniffi-bindgen
 * reads the metadata symbols that strips. The Android `.so` stays release —
 * bindings and cdylib are generated independently, and the uniffi contract
 * checksums are profile-independent.
 *
 * `armeabi-v7a` is in the ABI list. The app's release `splits` block produces an
 * `armeabi-v7a` APK, so omitting it here would ship a 32-bit APK with no routing
 * library and crash on the first route.
 */
plugins {
    id("turbo.android.library")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.routing"
    defaultConfig {
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
}

// ── Rust FFI: host bindgen + per-ABI Android cdylibs ───────────────────────
val tileserverDir = rootDir.resolveSibling("tileserver")
val hostLibFile = when {
    System.getProperty("os.name").lowercase().contains("mac") -> "libturbo_route_ffi.dylib"
    System.getProperty("os.name").lowercase().contains("win") -> "turbo_route_ffi.dll"
    else -> "libturbo_route_ffi.so"
}
// Generated into AGP's default source roots (gitignored) — picked up with no
// sourceSets wiring.
val ffiBindingsDir = projectDir.resolve("src/main/kotlin")
val ffiJniLibsDir = projectDir.resolve("src/main/jniLibs")

// Both ARM ABIs the release splits produce, plus x86_64 for emulators.
val androidAbis = listOf("arm64-v8a", "armeabi-v7a", "x86_64")

val ndkHome: String? = android.sdkDirectory.resolve("ndk")
    .listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }?.absolutePath

val buildRouteFfiHost = tasks.register<Exec>("buildRouteFfiHost") {
    group = "routing"
    description = "Build the host turbo-route-ffi cdylib (for binding generation)."
    workingDir = tileserverDir
    commandLine("cargo", "build", "-p", "turbo-route-ffi")
    outputs.file(tileserverDir.resolve("target/debug/$hostLibFile"))
    // The crate tree is large + shared; let cargo decide freshness (it's
    // incremental) rather than risk generating from a stale library.
    outputs.upToDateWhen { false }
}

val generateRouteFfiBindings = tasks.register<Exec>("generateRouteFfiBindings") {
    group = "routing"
    description = "Generate the Kotlin uniffi bindings from the host cdylib."
    dependsOn(buildRouteFfiHost)
    workingDir = tileserverDir
    commandLine(
        "cargo", "run", "-q", "-p", "turbo-route-ffi", "--bin", "uniffi-bindgen", "--",
        "generate",
        "--library", "target/debug/$hostLibFile",
        "--language", "kotlin",
        "--no-format",
        "--out-dir", ffiBindingsDir.absolutePath,
    )
    outputs.dir(ffiBindingsDir.resolve("uniffi"))
    outputs.upToDateWhen { false }
}

val buildRouteFfiAndroid = tasks.register<Exec>("buildRouteFfiAndroid") {
    group = "routing"
    description = "Cross-compile turbo-route-ffi for the Android ABIs into jniLibs."
    workingDir = tileserverDir
    if (ndkHome != null) environment("ANDROID_NDK_HOME", ndkHome)
    val abiArgs = androidAbis.flatMap { listOf("-t", it) }
    commandLine(
        listOf("cargo", "ndk") + abiArgs +
            listOf("-o", ffiJniLibsDir.absolutePath, "build", "--release", "-p", "turbo-route-ffi"),
    )
    outputs.dir(ffiJniLibsDir)
    outputs.upToDateWhen { false }
}

tasks.withType<KotlinCompile>().configureEach { dependsOn(generateRouteFfiBindings) }
tasks.named("preBuild") { dependsOn(generateRouteFfiBindings, buildRouteFfiAndroid) }

dependencies {
    // LatLng, RoutePlan, RoutePreset, RouteStreamEvent — the app's own vocabulary.
    implementation(project(":core:model"))
    // The `RouteRepository` seam this module provides a second implementation of.
    implementation(project(":core:data"))

    // JNA for Android (@aar bundles libjnidispatch.so for each ABI).
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
    implementation(libs.kotlinx.coroutines.android)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.core)
}

// Never lint generated bindings.
tasks.withType<io.gitlab.arturbosch.detekt.Detekt>().configureEach {
    exclude("**/uniffi/**")
}
