package com.sigmundgranaas.turbo.expressive.domain

/** Terrain vertical exaggeration the 3D slider dials in when first enabled — its
 *  default detent. Range is `[0, MAX_3D_EXAGGERATION]`; 0 == flat 2D. */
const val DEFAULT_3D_DETENT: Float = 6f

/** Upper bound of the 3D-terrain slider (vertical exaggeration over true scale). */
const val MAX_3D_EXAGGERATION: Float = 8f

/** Daylight arc the Sun slider sweeps: `sunLevel` 0→1 maps to this hour range so
 *  dragging the slider rakes the sun (and its shadows) from dawn to dusk. */
private const val SUN_HOUR_DAWN: Float = 4f
private const val SUN_HOUR_DUSK: Float = 22f

/**
 * The map's derived scene environment — computed purely from the two layers-sheet
 * sliders (3D terrain + Sun). This is the tested seam behind Phase 1's "control
 * center": every rule the spec pins down about what the sliders do (and, crucially,
 * what they must NOT do) lives here as a pure function.
 *
 * **The hard decoupling rule** is structural, not a runtime check: this type carries
 * NO camera field. Neither slider can move the camera because the reducer has nothing
 * to move it with — [tiltEnabled] only says whether *gestures* may tilt, it never
 * forces a pitch. That kills the old "enabling sun snaps you into a 3D tilt."
 */
data class MapEnvironment(
    /** The DEM heightfield is loaded (needed for 3D relief *and* for 2D sun-lit relief). */
    val demPresent: Boolean,
    /** Vertical exaggeration applied to the terrain mesh; 0 whenever the map is flat 2D. */
    val exaggeration: Float,
    /** Normalized sun slider `[0, 1]`; 0 == no relief lighting. */
    val sunLevel: Float,
    /** Sun position as an hour-of-day in `[SUN_HOUR_DAWN, SUN_HOUR_DUSK]`, or null when
     *  the sun is off. Moving the slider moves this — the "sun vector" the scene lights by. */
    val sunHour: Float?,
    /** Whether pitch/orbit gestures are accepted. True only in 3D; the sun slider never
     *  unlocks tilt (2D sun is lit strictly top-down). */
    val tiltEnabled: Boolean,
)

/**
 * Derive the [MapEnvironment] from the two slider levels.
 *
 * @param threeDLevel terrain exaggeration `[0, MAX_3D_EXAGGERATION]`; 0 == flat 2D.
 * @param sunLevel sun position `[0, 1]`; 0 == sun off.
 */
fun mapEnvironment(threeDLevel: Float, sunLevel: Float): MapEnvironment {
    val td = threeDLevel.coerceIn(0f, MAX_3D_EXAGGERATION)
    val sun = sunLevel.coerceIn(0f, 1f)
    val threeD = td > 0f
    val sunOn = sun > 0f
    return MapEnvironment(
        // Sun-lit relief in 2D needs the DEM too — this is what lets the sun slider
        // work in both modes without ever tilting the camera.
        demPresent = threeD || sunOn,
        // The mesh must have real vertical relief for the sun to have slopes to light.
        // In 2D-sun that relief is present but viewed top-down (tilt stays locked) —
        // literally "3D seen from the top". 3D uses the slider's own value.
        exaggeration = when {
            threeD -> td
            sunOn -> DEFAULT_3D_DETENT
            else -> 0f
        },
        sunLevel = sun,
        sunHour = if (sunOn) SUN_HOUR_DAWN + sun * (SUN_HOUR_DUSK - SUN_HOUR_DAWN) else null,
        // Only the 3D slider unlocks tilt; the sun slider never does (top-down-lit).
        tiltEnabled = threeD,
    )
}
