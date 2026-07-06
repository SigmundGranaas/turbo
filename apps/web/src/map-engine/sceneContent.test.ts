/** P6.3 gate: the content plane (route lines, pins, the location fix) emits
 *  scene-declared geo-json sources + content layers in exactly the serde
 *  shape the engine parses (`turbomap-scene`: tag="type" kebab-case variants,
 *  snake_case fields, `{r,g,b,a}` colors). Pins the JSON so a drive-by
 *  refactor can't silently de-declare map content. */
import { beforeEach, describe, expect, it } from 'vitest';
import { setMapContent, setMapLine } from '../map-core';
import { buildBaseScene, cssColorToScene, type Layer } from './scene';

function reset() {
  setMapContent({ lines: {}, pins: [], selectedPinId: undefined, userFix: null });
}

function layer(scene: { layers: Layer[] }, id: string): Layer | undefined {
  return scene.layers.find((l) => l.id === id);
}

describe('scene-declared map content (P6.3)', () => {
  beforeEach(reset);

  it('emits a route line as a geo-json source with halo + stroke line layers', () => {
    setMapLine('route', {
      coords: [
        { lat: 60.39, lng: 5.32 },
        { lat: 60.4, lng: 5.33 },
      ],
      dashed: true,
      color: '#e53935',
    });
    const scene = buildBaseScene('norgeskart');
    const src = scene.sources['content-line-route'];
    expect(src).toBeDefined();
    expect(src.type).toBe('geo-json');
    expect(JSON.parse((src as { data: string }).data)).toEqual({
      type: 'LineString',
      coordinates: [
        [5.32, 60.39],
        [5.33, 60.4],
      ],
    });
    const halo = layer(scene, 'content-line-route-halo');
    const stroke = layer(scene, 'content-line-route');
    expect(halo).toMatchObject({ type: 'line', width: 9 });
    expect(stroke).toMatchObject({
      type: 'line',
      width: 5,
      dash_array: [2, 10],
      color: { r: 0xe5, g: 0x39, b: 0x35, a: 255 },
    });
    // Serde shape sanity: the wire JSON tags variants via `type`.
    expect(JSON.parse(JSON.stringify(stroke)).type).toBe('line');
  });

  it('a line needs two points, and clearing an owner removes exactly its line', () => {
    setMapLine('route', { coords: [{ lat: 1, lng: 1 }] });
    expect(buildBaseScene('norgeskart').sources['content-line-route']).toBeUndefined();
    setMapLine('route', {
      coords: [
        { lat: 1, lng: 1 },
        { lat: 2, lng: 2 },
      ],
    });
    setMapLine('track', {
      coords: [
        { lat: 3, lng: 3 },
        { lat: 4, lng: 4 },
      ],
    });
    setMapLine('route', null);
    const scene = buildBaseScene('norgeskart');
    expect(scene.sources['content-line-route']).toBeUndefined();
    expect(scene.sources['content-line-track']).toBeDefined();
  });

  it('emits pins grouped by kind colour with the selected pin emphasized on top', () => {
    setMapContent({
      pins: [
        { id: 'a', lat: 60, lng: 5, color: '#388E3C' },
        { id: 'b', lat: 61, lng: 6, color: '#388E3C' },
        { id: 'c', lat: 62, lng: 7, color: '#F57C00' },
      ],
      selectedPinId: 'c',
    });
    const scene = buildBaseScene('norgeskart');
    // Two unselected of one colour → one FeatureCollection source (each pin a
    // feature carrying its domain id for hit-testing); selected 'c' gets its
    // own emphasized pair.
    const group = scene.sources['content-pins-0'];
    const fc = JSON.parse((group as { data: string }).data);
    expect(fc.type).toBe('FeatureCollection');
    expect(fc.features.map((f: { properties: { id: string } }) => f.properties.id)).toEqual(['a', 'b']);
    expect(fc.features[0].geometry).toEqual({ type: 'Point', coordinates: [5, 60] });
    expect(layer(scene, 'content-pins-0-ring')).toMatchObject({ type: 'circle', radius: 11 });
    expect(layer(scene, 'content-pins-0')).toMatchObject({
      type: 'circle',
      radius: 8,
      color: { r: 0x38, g: 0x8e, b: 0x3c, a: 255 },
    });
    expect(layer(scene, 'content-pin-selected')).toMatchObject({
      type: 'circle',
      radius: 11,
      color: { r: 0xf5, g: 0x7c, b: 0x00, a: 255 },
    });
    // Selected pin draws AFTER (above) its group.
    const ids = scene.layers.map((l) => l.id);
    expect(ids.indexOf('content-pin-selected')).toBeGreaterThan(ids.indexOf('content-pins-0'));
  });

  it('emits the user-location fix as halo + ring + dot circles', () => {
    setMapContent({ userFix: { lat: 60.39, lng: 5.32 } });
    const scene = buildBaseScene('norgeskart');
    expect(layer(scene, 'content-user-location-halo')).toMatchObject({
      type: 'circle',
      radius: 22,
      color: { r: 37, g: 99, b: 235, a: 46 },
    });
    expect(layer(scene, 'content-user-location')).toMatchObject({ type: 'circle', radius: 8 });
    // Clearing the fix removes the dot from the next scene.
    setMapContent({ userFix: null });
    expect(layer(buildBaseScene('norgeskart'), 'content-user-location')).toBeUndefined();
  });

  it('resolves CSS colors defensively', () => {
    expect(cssColorToScene('#102030')).toEqual({ r: 0x10, g: 0x20, b: 0x30, a: 255 });
    // Unresolvable var() in a non-DOM environment falls back, never throws.
    expect(cssColorToScene('var(--primary)')).toEqual({ r: 37, g: 99, b: 235, a: 255 });
    expect(cssColorToScene(undefined)).toEqual({ r: 37, g: 99, b: 235, a: 255 });
  });
});
