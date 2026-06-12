import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

/**
 * `:core:turbomap-android` — the on-device half of the wgpu engine binding
 * (Stage C of the Android renderer swap, see
 * `docs/architecture/2026-06-android-renderer-swap-test-plan.md`).
 *
 * Kept separate from the host-JVM `:core:turbomap` (Stage B) on purpose: that
 * module loads the *host* cdylib via the desktop JNA jar; this one packages the
 * per-ABI Android `.so` (cargo-ndk) and uses the JNA **@aar** (whose own
 * `libjnidispatch.so` is built for Android). Mixing the two JNA artifacts in one
 * module is the classic uniffi-on-Android footgun, so we don't.
 *
 * The instrumented test runs the engine on the device's real GPU (Vulkan/GLES)
 * through the generated bindings — proving the whole native stack works on
 * Android: cargo-ndk `.so` → APK jniLibs → JNA on ART → uniffi → wgpu pixels.
 * The on-screen `SurfaceView`/`ANativeWindow`/Choreographer glue is the next
 * increment; this lands the foundation it stands on.
 *
 * The cdylibs + bindings are generated into AGP's default source locations
 * (`src/main/kotlin`, `src/main/jniLibs`, both gitignored) so we don't fight the
 * AGP 9 new-DSL sourceSet API — generation is wired as a compile/preBuild dep.
 */
plugins {
    id("turbo.android.library")
    id("turbo.android.compose")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.core.turbomap.android"
    defaultConfig {
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
}

// ── Rust FFI: host bindgen + per-ABI Android cdylibs ───────────────────────
val turbomapDir = rootDir.resolveSibling("turbomap")
val hostLibFile = when {
    System.getProperty("os.name").lowercase().contains("mac") -> "libturbomap_ffi.dylib"
    System.getProperty("os.name").lowercase().contains("win") -> "turbomap_ffi.dll"
    else -> "libturbomap_ffi.so"
}
// Generated into AGP's default source roots (gitignored) — picked up with no
// sourceSets wiring.
val ffiBindingsDir = projectDir.resolve("src/main/kotlin")
val ffiJniLibsDir = projectDir.resolve("src/main/jniLibs")

// arm64-v8a covers Apple-silicon emulators + modern phones; x86_64 covers Intel
// emulators. (Release adds armeabi-v7a in a later stage.)
val androidAbis = listOf("arm64-v8a", "x86_64")

// cargo-ndk needs the NDK location; derive it from the configured SDK so no
// machine-specific path is baked in.
val ndkHome: String? = android.sdkDirectory.resolve("ndk")
    .listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }?.absolutePath

// Kotlin bindings are generated from the HOST cdylib (uniffi-bindgen loads the
// library to read its metadata, so it must match the build machine's arch).
val buildRustFfiHost = tasks.register<Exec>("buildRustFfiHost") {
    group = "turbomap"
    description = "Build the host turbomap-ffi cdylib (for binding generation)."
    workingDir = turbomapDir
    commandLine("cargo", "build", "-p", "turbomap-ffi")
    outputs.file(turbomapDir.resolve("target/debug/$hostLibFile"))
    // The crate tree is large + shared; let cargo decide freshness (it's
    // incremental) rather than risk packaging a stale .so on a source change.
    outputs.upToDateWhen { false }
}

val generateFfiBindings = tasks.register<Exec>("generateFfiBindings") {
    group = "turbomap"
    description = "Generate the Kotlin uniffi bindings from the host cdylib."
    dependsOn(buildRustFfiHost)
    workingDir = turbomapDir
    commandLine(
        "cargo", "run", "-q", "-p", "turbomap-ffi", "--bin", "uniffi-bindgen", "--",
        "generate",
        "--library", "target/debug/$hostLibFile",
        "--language", "kotlin",
        "--no-format",
        "--out-dir", ffiBindingsDir.absolutePath,
    )
    outputs.dir(ffiBindingsDir.resolve("uniffi"))
    outputs.upToDateWhen { false }
}

val buildRustFfiAndroid = tasks.register<Exec>("buildRustFfiAndroid") {
    group = "turbomap"
    description = "Cross-compile turbomap-ffi for the Android ABIs into jniLibs."
    workingDir = turbomapDir
    if (ndkHome != null) environment("ANDROID_NDK_HOME", ndkHome)
    val abiArgs = androidAbis.flatMap { listOf("-t", it) }
    commandLine(
        listOf("cargo", "ndk") + abiArgs +
            listOf("-o", ffiJniLibsDir.absolutePath, "build", "-p", "turbomap-ffi"),
    )
    outputs.dir(ffiJniLibsDir)
    outputs.upToDateWhen { false }
}

tasks.withType<KotlinCompile>().configureEach { dependsOn(generateFfiBindings) }
// Ensure bindings + .so exist before AGP compiles / merges jniLibs into the APK.
tasks.named("preBuild") { dependsOn(generateFfiBindings, buildRustFfiAndroid) }

dependencies {
    // The renderer-agnostic MapEngine contract + LatLng/GeoBounds (no MapLibre).
    implementation(project(":core:model"))
    // Re-exports the Compose deps (ui/foundation) for the on-screen host.
    implementation(project(":core:designsystem"))

    // JNA for Android (@aar bundles libjnidispatch.so for each ABI).
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

    // The on-screen Compose host (SurfaceView + Choreographer + host-driven tiles).
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.kotlinx.coroutines.android)

    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.core)
}

// Never lint generated bindings.
tasks.withType<io.gitlab.arturbosch.detekt.Detekt>().configureEach {
    exclude("**/uniffi/**")
}
