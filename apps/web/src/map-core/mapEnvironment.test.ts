import { describe, expect, it } from 'vitest';
import { deriveMapEnvironment, DEFAULT_3D_DETENT, MAX_3D_EXAGGERATION } from './mapEnvironment';

/**
 * Phase 1 "scene-IR" tests, mirrored 1:1 from Android's `MapEnvironmentTest.kt`:
 * the layers-sheet 3D + Sun sliders drive the map's environment through a pure
 * reducer, so we can assert the user-facing rules (relief appears, tilt unlocks,
 * the sun moves) without a device or a GPU.
 */
describe('deriveMapEnvironment (layers-sheet 3D + Sun sliders)', () => {
  it('3D at zero is flat 2D — no DEM, tilt locked', () => {
    const env = deriveMapEnvironment(0, 0);
    expect(env.demPresent).toBe(false); // no terrain loaded when flat
    expect(env.exaggeration).toBe(0); // no exaggeration when flat
    expect(env.tiltEnabled).toBe(false); // pitch gestures rejected in 2D
  });

  it('3D above zero shows terrain and unlocks tilt', () => {
    const env = deriveMapEnvironment(DEFAULT_3D_DETENT, 0);
    expect(env.demPresent).toBe(true); // DEM present in 3D
    expect(env.exaggeration).toBe(DEFAULT_3D_DETENT); // exaggeration carries the slider value
    expect(env.tiltEnabled).toBe(true); // pitch gestures accepted in 3D
  });

  it('sun in 2D lights relief top-down — DEM, relief, and sun, but tilt stays locked', () => {
    const env = deriveMapEnvironment(0, 0.5);
    expect(env.demPresent).toBe(true); // sun needs the DEM to light relief
    expect(env.sunHour).not.toBeNull(); // sun vector present
    // "3D seen from the top": the mesh has relief for the sun to shade, but the
    // camera never tilts — that's what keeps it a 2D map.
    expect(env.exaggeration).toBeGreaterThan(0); // relief present so the sun has slopes to light
    expect(env.tiltEnabled).toBe(false); // the sun slider must NOT unlock tilt
  });

  it('moving the sun slider moves the sun — decoupled from any camera field', () => {
    const a = deriveMapEnvironment(0, 0.25);
    const b = deriveMapEnvironment(0, 0.75);
    expect(a.sunHour).not.toBe(b.sunHour); // sun position changes with the slider
    // Decoupling is structural: neither environment carries a camera/pitch field
    // to change, and tilt stays locked in both (2D) — the slider can't tilt the view.
    expect(a.tiltEnabled).toBe(false);
    expect(b.tiltEnabled).toBe(false);
    expect('camera' in a).toBe(false); // the reducer carries NO camera field
  });

  it('sun off yields no sun vector', () => {
    expect(deriveMapEnvironment(0, 0).sunHour).toBeNull(); // no sun vector when the slider is at 0
  });

  it('3D and sun together — terrain, exaggeration, sun, and tilt', () => {
    const env = deriveMapEnvironment(4, 1);
    expect(env.demPresent).toBe(true);
    expect(env.exaggeration).toBe(4);
    expect(env.sunHour).not.toBeNull(); // sun on
    expect(env.tiltEnabled).toBe(true);
  });

  it('slider values are clamped to their ranges', () => {
    const hi = deriveMapEnvironment(99, 9);
    expect(hi.exaggeration).toBe(MAX_3D_EXAGGERATION); // 3D clamps to the max exaggeration
    expect(hi.sunLevel).toBe(1); // sun clamps to 1
    const lo = deriveMapEnvironment(-5, -5);
    expect(lo.demPresent).toBe(false); // negative 3D is flat
    expect(lo.sunLevel).toBe(0);
  });
});
