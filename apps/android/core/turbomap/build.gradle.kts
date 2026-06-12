import org.jetbrains.kotlin.gradle.dsl.JvmTarget

/**
 * `:core:turbomap` — the host-side binding for the wgpu/Rust map engine
 * (`apps/turbomap`, crate `turbomap-ffi`), exposed to Kotlin via uniffi.
 *
 * Stage B of the Android renderer swap (see
 * `docs/architecture/2026-06-android-renderer-swap-test-plan.md`): a plain
 * Kotlin/JVM module so its unit tests **execute the engine** on the host GPU
 * (Metal on a dev Mac, Lavapipe in CI) through the generated bindings + JNA —
 * proving the Rust work actually reaches Kotlin. No surface glue here; the
 * on-screen path and per-ABI Android `.so` packaging arrive in Stage C, when
 * this becomes the `MapEngine` the app links against.
 *
 * The cdylib is built and the Kotlin bindings are generated **at build time**
 * from the same library that is loaded, so the uniffi contract checksums can
 * never drift from a stale committed artifact.
 */
plugins {
    alias(libs.plugins.kotlin.jvm)
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

// ── Rust FFI: build the cdylib + generate Kotlin bindings ──────────────────
// apps/turbomap is a sibling of apps/android (this build's rootDir).
val turbomapDir = rootDir.resolveSibling("turbomap")
val ffiTargetDir = turbomapDir.resolve("target/debug")
val ffiLibFile = when {
    System.getProperty("os.name").lowercase().contains("mac") -> "libturbomap_ffi.dylib"
    System.getProperty("os.name").lowercase().contains("win") -> "turbomap_ffi.dll"
    else -> "libturbomap_ffi.so"
}
val ffiBindingsDir = layout.buildDirectory.dir("generated/source/ffi")

// cargo is incremental, so these run cheaply when nothing changed. We don't
// declare fine-grained inputs (the crate tree is large) — correctness comes
// from cargo, and this module isn't on the default app/CI compile path.
val buildRustFfi = tasks.register<Exec>("buildRustFfi") {
    group = "turbomap"
    description = "Build the turbomap-ffi cdylib for the host triple."
    workingDir = turbomapDir
    commandLine("cargo", "build", "-p", "turbomap-ffi")
    outputs.file(ffiTargetDir.resolve(ffiLibFile))
    outputs.upToDateWhen { false } // let cargo (incremental) decide freshness
}

val generateFfiBindings = tasks.register<Exec>("generateFfiBindings") {
    group = "turbomap"
    description = "Generate the Kotlin uniffi bindings from the built cdylib."
    dependsOn(buildRustFfi)
    workingDir = turbomapDir
    commandLine(
        "cargo", "run", "-q", "-p", "turbomap-ffi", "--bin", "uniffi-bindgen", "--",
        "generate",
        "--library", "target/debug/$ffiLibFile",
        "--language", "kotlin",
        "--no-format",
        "--out-dir", ffiBindingsDir.get().asFile.absolutePath,
    )
    outputs.dir(ffiBindingsDir)
    outputs.upToDateWhen { false }
}

// The generated sources compile as part of main; the TaskProvider wires the
// build dependency so `compileKotlin` triggers generation automatically.
sourceSets.named("main") {
    java.srcDir(generateFfiBindings)
}

dependencies {
    // Domain types (LatLng) the scene builder maps from — renderer-agnostic.
    implementation(project(":core:model"))

    // Backs the generated bindings' native calls.
    implementation(libs.jna)

    testImplementation(libs.junit)
}

// The host-GPU FFI round-trip suite (Lane D). JNA resolves `turbomap_ffi` to
// the freshly built cdylib via jna.library.path.
tasks.named<Test>("test") {
    dependsOn(buildRustFfi)
    systemProperty("jna.library.path", ffiTargetDir.absolutePath)
    testLogging { events("passed", "skipped", "failed") }
}

// Never lint generated bindings.
tasks.withType<io.gitlab.arturbosch.detekt.Detekt>().configureEach {
    exclude("**/generated/**")
}
