package com.sigmundgranaas.turbo.expressive.feature.map.core

import androidx.compose.runtime.staticCompositionLocalOf
import com.sigmundgranaas.turbo.expressive.domain.MapEngine

/**
 * A pre-built [MapEngine] for environments that cannot host the native wgpu
 * surface — Compose `@Preview` and headless Robolectric tests.
 *
 * When non-null, the map host skips building the `SurfaceView` + native engine
 * and drives the map through this engine instead, so the entire screen (and every
 * tool that talks to the renderer-agnostic [MapEngine] seam) is exercisable
 * without a GPU. **Null in production** — the real engine is built from the live
 * surface — so this changes nothing on a real device.
 */
val LocalMapEngineOverride = staticCompositionLocalOf<MapEngine?> { null }
