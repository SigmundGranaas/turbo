/** The compass auto-hide rule (spec Phase 1): the reset-north compass is hidden
 *  while the map is within ~0.5° of north and appears once it's rotated. A pure
 *  boundary rule — no React, no pixels — so it's testable directly. Mirrors
 *  Android's `compassVisible`. */
export function compassVisible(bearingDeg: number): boolean {
  return Math.abs(bearingDeg) > 0.5;
}
