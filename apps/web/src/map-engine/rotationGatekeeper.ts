/**
 * Decides — once per two-finger gesture — whether the gesture rotates the map or
 * pans-and-zooms it, and LOCKS that verdict for the rest of the gesture (the
 * "sequence lock" from the spec). Rotation engages only when finger-twist is the
 * *primary* movement: the accumulated twist must cross [rotationGateDeg] while
 * neither pinch nor pan has crossed its own threshold. If pinch or pan leads (or
 * ties in the same frame), the gesture is pan-and-zoom and rotation is suppressed
 * for its whole duration — so a natural pinch never wobbles the bearing.
 *
 * A faithful port of Android's `RotationGatekeeper` (spec Phase 1 tail). Pure —
 * fed accumulated deltas measured from gesture start, no React, no pointer. This
 * is the seam the 2D twist-rotate gesture and the compass "Lock rotation" setting
 * both drive. See docs/architecture/2026-07-turbo-map-overhaul-spec.md.
 */
export type TwoFingerVerdict = 'undecided' | 'rotate' | 'pan-zoom';

export const DEFAULT_ROTATION_GATE_DEG = 10;
/** Pinch magnitude (|ratio − 1|) that counts as "zoom led". ~4%. */
export const DEFAULT_PINCH_THRESHOLD = 0.04;
/** Centroid travel (px) that counts as "pan led" — the platform touch slop. */
export const DEFAULT_PAN_SLOP_PX = 24;

export interface RotationGatekeeperOptions {
  rotationGateDeg?: number;
  pinchThreshold?: number;
  panSlopPx?: number;
  /** When true (compass lock / rotation disabled), rotation never engages —
   *  every two-finger gesture is pan-and-zoom. */
  rotationLocked?: boolean;
}

export class RotationGatekeeper {
  private verdict: TwoFingerVerdict = 'undecided';
  private readonly rotationGateDeg: number;
  private readonly pinchThreshold: number;
  private readonly panSlopPx: number;
  private readonly rotationLocked: boolean;

  constructor(opts: RotationGatekeeperOptions = {}) {
    this.rotationGateDeg = opts.rotationGateDeg ?? DEFAULT_ROTATION_GATE_DEG;
    this.pinchThreshold = opts.pinchThreshold ?? DEFAULT_PINCH_THRESHOLD;
    this.panSlopPx = opts.panSlopPx ?? DEFAULT_PAN_SLOP_PX;
    this.rotationLocked = opts.rotationLocked ?? false;
  }

  /** The locked-in verdict (or `'undecided'` before either threshold is crossed). */
  get current(): TwoFingerVerdict {
    return this.verdict;
  }

  /**
   * Fold in the gesture's cumulative state since it started:
   * `twistDegFromStart` net finger-pair rotation (signed degrees),
   * `pinchRatioFromStart` current spread ÷ start spread (1 = no pinch),
   * `panPxFromStart` centroid displacement from the start point (px).
   * Returns the (possibly newly-locked) verdict.
   */
  update(twistDegFromStart: number, pinchRatioFromStart: number, panPxFromStart: number): TwoFingerVerdict {
    if (this.verdict !== 'undecided') return this.verdict;
    if (this.rotationLocked) {
      // Locked: only pan/zoom can ever engage; twist is inert.
      if (Math.abs(pinchRatioFromStart - 1) >= this.pinchThreshold || panPxFromStart >= this.panSlopPx) {
        this.verdict = 'pan-zoom';
      }
      return this.verdict;
    }
    const twistCrossed = Math.abs(twistDegFromStart) >= this.rotationGateDeg;
    const pinchCrossed = Math.abs(pinchRatioFromStart - 1) >= this.pinchThreshold;
    const panCrossed = panPxFromStart >= this.panSlopPx;
    if (twistCrossed && !pinchCrossed && !panCrossed) {
      // Rotation only if twist LEADS — clean of pinch and pan.
      this.verdict = 'rotate';
    } else if (pinchCrossed || panCrossed) {
      // A same-frame twist+pinch/pan tie falls here → pan-zoom, the conservative choice.
      this.verdict = 'pan-zoom';
    }
    return this.verdict;
  }
}

/** Angle (deg) of the vector from finger A to finger B. */
export function pairAngleDeg(ax: number, ay: number, bx: number, by: number): number {
  return (Math.atan2(by - ay, bx - ax) * 180) / Math.PI;
}

/** Signed per-frame twist (deg), wrapped to (−180, 180] so a twist across the
 *  ±180° seam doesn't spike. */
export function twistDeltaDeg(prevDeg: number, deg: number): number {
  let d = deg - prevDeg;
  while (d > 180) d -= 360;
  while (d <= -180) d += 360;
  return d;
}
