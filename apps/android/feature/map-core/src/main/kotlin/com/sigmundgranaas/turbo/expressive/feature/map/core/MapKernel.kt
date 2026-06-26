package com.sigmundgranaas.turbo.expressive.feature.map.core

import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * The passive kernel of the map tier.
 *
 * It holds only the renderer-agnostic *contracts* a map tool needs from its host
 * — never orchestration (that lives in the host's `MapHostCoordinator`) and never
 * tool-specific state. Tool modules (`feature:map-*`) depend on this; they never
 * depend on each other or on the host. Cross-tool behaviour flows through `core:*`
 * seams (e.g. `FollowController` in `core:data`), not through this kernel.
 *
 * See `docs/architecture/2026-06-android-architecture-remediation-plan.md`.
 */

/** Read-only camera snapshot a tool may observe (e.g. to label "centre here"). */
data class MapCameraState(
    val center: LatLng,
    val zoom: Double,
    val pitchDeg: Float = 0f,
)

/**
 * The callbacks a map tool may invoke on its host. The host (`feature:map`)
 * implements this and hands it to each tool, so a tool can drive the shared map
 * (move the camera, dismiss itself) without depending on the host's concrete
 * screen or on any sibling tool.
 */
interface MapToolHost {
    /** Animate the shared map camera to [target] at [zoom]. */
    fun flyTo(target: LatLng, zoom: Double)

    /** Close whichever tool is currently active on the map. */
    fun closeActiveTool()
}
