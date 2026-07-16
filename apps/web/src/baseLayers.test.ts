import { describe, it, expect } from 'vitest';
import { isValidXyzTemplate } from './baseLayers';

/** Adding a custom map (the user's goal): a working XYZ template is accepted,
 *  anything the engine couldn't substitute tiles into is rejected up front.
 *  Mirrors the Android CustomTileSource rule so both clients agree. */
describe('isValidXyzTemplate', () => {
  it('accepts http(s) templates with all three placeholders', () => {
    expect(isValidXyzTemplate('https://example.com/tiles/{z}/{x}/{y}.png')).toBe(true);
    expect(isValidXyzTemplate('  http://tiles.local/{z}/{y}/{x}  ')).toBe(true);
    expect(isValidXyzTemplate('https://mt1.example.com/vt?x={x}&y={y}&z={z}')).toBe(true);
  });

  it('rejects missing placeholders and bad schemes', () => {
    expect(isValidXyzTemplate('https://example.com/tiles/{z}/{x}.png')).toBe(false);
    expect(isValidXyzTemplate('ftp://example.com/{z}/{x}/{y}.png')).toBe(false);
    expect(isValidXyzTemplate('example.com/{z}/{x}/{y}.png')).toBe(false);
    expect(isValidXyzTemplate('')).toBe(false);
  });
});
