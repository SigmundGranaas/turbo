/** TypeScript mirror of Android's `MapEnvironment.kt`
 *  (`com.sigmundgranaas.turbo.expressive.domain.mapEnvironment`). The map's
 *  derived scene environment, computed purely from the two layers-sheet sliders
 *  (3D terrain + Sun). Every rule Phase 1 pins down about what the sliders do —
 *  and, crucially, what they must NOT do — lives here as a pure function so it's
 *  testable without a device or a GPU.
 *
 *  **The hard decoupling rule** is structural, not a runtime check: [DerivedEnv]
 *  carries NO camera field. Neither slider can move the camera because the
 *  reducer has nothing to move it with — [tiltEnabled] only says whether
 *  *gestures* may tilt, it never forces a pitch. That kills the old "enabling
 *  sun snaps you into a 3D tilt." */

/** Terrain vertical exaggeration the 3D slider dials in when first enabled — its
 *  default detent. Range is `[0, MAX_3D_EXAGGERATION]`; 0 == flat 2D. */
export const DEFAULT_3D_DETENT = 6;

/** Upper bound of the 3D-terrain slider (vertical exaggeration over true scale). */
export const MAX_3D_EXAGGERATION = 8;

/** Daylight arc the Sun slider sweeps: `sunLevel` 0→1 maps to this hour range so
 *  dragging the slider rakes the sun (and its shadows) from dawn to dusk. */
const SUN_HOUR_DAWN = 4;
const SUN_HOUR_DUSK = 22;

export interface DerivedEnv {
  /** The DEM heightfield is loaded (needed for 3D relief *and* for 2D sun-lit relief). */
  demPresent: boolean;
  /** Vertical exaggeration applied to the terrain mesh; 0 whenever the map is flat 2D. */
  exaggeration: number;
  /** Normalized sun slider `[0, 1]`; 0 == no relief lighting. */
  sunLevel: number;
  /** Sun position as an hour-of-day in `[SUN_HOUR_DAWN, SUN_HOUR_DUSK]`, or null
   *  when the sun is off. Moving the slider moves this — the "sun vector" the
   *  scene lights by. */
  sunHour: number | null;
  /** Whether pitch/orbit gestures are accepted. True only in 3D; the sun slider
   *  never unlocks tilt (2D sun is lit strictly top-down). */
  tiltEnabled: boolean;
}

const clamp = (v: number, lo: number, hi: number): number => Math.min(Math.max(v, lo), hi);

/**
 * Derive the {@link DerivedEnv} from the two slider levels.
 *
 * @param threeDLevel terrain exaggeration `[0, MAX_3D_EXAGGERATION]`; 0 == flat 2D.
 * @param sunLevel sun position `[0, 1]`; 0 == sun off.
 */
export function deriveMapEnvironment(threeDLevel: number, sunLevel: number): DerivedEnv {
  const td = clamp(threeDLevel, 0, MAX_3D_EXAGGERATION);
  const sun = clamp(sunLevel, 0, 1);
  const threeD = td > 0;
  const sunOn = sun > 0;
  return {
    // Sun-lit relief in 2D needs the DEM too — this is what lets the sun slider
    // work in both modes without ever tilting the camera.
    demPresent: threeD || sunOn,
    // The mesh must have real vertical relief for the sun to have slopes to
    // light. In 2D-sun that relief is present but viewed top-down (tilt stays
    // locked) — literally "3D seen from the top". 3D uses the slider's own value.
    exaggeration: threeD ? td : sunOn ? DEFAULT_3D_DETENT : 0,
    sunLevel: sun,
    sunHour: sunOn ? SUN_HOUR_DAWN + sun * (SUN_HOUR_DUSK - SUN_HOUR_DAWN) : null,
    // Only the 3D slider unlocks tilt; the sun slider never does (top-down-lit).
    tiltEnabled: threeD,
  };
}
