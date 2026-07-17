import { describe, it, expect } from 'vitest';
import { RotationGatekeeper, twistDeltaDeg } from './rotationGatekeeper';

/**
 * The rotation gatekeeper decides — and LOCKS — whether a two-finger gesture
 * rotates or pans-and-zooms, so a natural pinch never wobbles the bearing and a
 * deliberate twist does rotate. Driven with synthetic cumulative deltas (no
 * React, no touch), which is the whole point of the pure extraction: the gesture
 * *decision* is testable without a device.
 */
describe('rotation gatekeeper', () => {
  const panSlopPx = 24;

  it('engages rotation when twist leads with no meaningful pinch/pan', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(4, 1.0, 2)).toBe('undecided');
    expect(gate.update(8, 1.0, 3)).toBe('undecided');
    expect(gate.update(12, 1.0, 4)).toBe('rotate');
  });

  it('suppresses rotation for the whole gesture when a pinch crosses first', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(1, 1.06, 2)).toBe('pan-zoom');
    // Later big twist must NOT flip it — the sequence is locked.
    expect(gate.update(40, 1.1, 5)).toBe('pan-zoom');
  });

  it('is pan-and-zoom (not rotation) when a centroid pan crosses first', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(2, 1.0, 30)).toBe('pan-zoom');
  });

  it('falls back to pan-zoom on a same-frame twist-and-pinch tie', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(12, 1.05, 1)).toBe('pan-zoom');
  });

  it('keeps an engaged rotation locked through later pinch and pan', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(12, 1.0, 2)).toBe('rotate');
    expect(gate.update(20, 1.3, 80)).toBe('rotate');
  });

  it('engages rotation on a counter-clockwise (negative) twist too', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(gate.update(-11, 1.0, 3)).toBe('rotate');
  });

  it('rejects a twist under a stricter gate that the default would accept', () => {
    const strict = new RotationGatekeeper({ rotationGateDeg: 20, panSlopPx });
    expect(strict.update(15, 1.0, 2)).toBe('undecided');
    const dflt = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx });
    expect(dflt.update(15, 1.0, 2)).toBe('rotate');
  });

  it('never engages rotation when locked, even on a clean twist', () => {
    const gate = new RotationGatekeeper({ rotationGateDeg: 10, panSlopPx, rotationLocked: true });
    expect(gate.update(30, 1.0, 1)).toBe('undecided');
    // A pinch still resolves it to pan-zoom.
    expect(gate.update(30, 1.06, 1)).toBe('pan-zoom');
  });
});

/** Per-frame twist wraps across the ±180° seam so a rotation past it doesn't spike. */
describe('twistDeltaDeg seam wrapping', () => {
  it('wraps a jump across +180° to a small negative step', () => {
    expect(twistDeltaDeg(179, -179)).toBeCloseTo(2, 5);
  });
  it('wraps a jump across −180° to a small positive step', () => {
    expect(twistDeltaDeg(-179, 179)).toBeCloseTo(-2, 5);
  });
  it('leaves an ordinary small step unchanged', () => {
    expect(twistDeltaDeg(10, 14)).toBeCloseTo(4, 5);
  });
});
