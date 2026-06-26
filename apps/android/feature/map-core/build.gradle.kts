plugins {
    id("turbo.android.library")
    id("turbo.android.compose")
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive.feature.map.core"
}

dependencies {
    // The renderer-agnostic seam tools talk to; the kernel never names a renderer.
    api(project(":core:model"))
    // Compose runtime (for the map-host CompositionLocal seam) via the design system's
    // re-exported Compose surface. designsystem is a core module, so the kernel still
    // depends on nothing in the feature tier.
    implementation(project(":core:designsystem"))
}
