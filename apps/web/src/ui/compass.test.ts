import { describe, it, expect } from 'vitest';
import { compassVisible } from './compass';

/**
 * The compass auto-hide rule: hidden when the map is within ~0.5° of north,
 * visible once it's rotated. A pure boundary test — no React, no pixels.
 */
describe('compass visibility', () => {
  it('hides near north', () => {
    expect(compassVisible(0)).toBe(false);
    expect(compassVisible(0.3)).toBe(false);
    expect(compassVisible(-0.3)).toBe(false);
  });

  it('shows on a rotated map, in either direction', () => {
    expect(compassVisible(15)).toBe(true);
    expect(compassVisible(-90)).toBe(true);
    expect(compassVisible(179)).toBe(true);
  });

  it('flips to visible just past the half-degree threshold', () => {
    expect(compassVisible(0.5)).toBe(false);
    expect(compassVisible(0.6)).toBe(true);
  });
});
