import { describe, it, expect } from 'vitest';
import { serializeMarkers, type Marker } from './api';

const camp: Marker = {
  id: 'm1',
  lat: 69.6,
  lng: 20.0,
  name: 'Camp',
  description: 'by the river',
  icon: 'camping',
  version: 1,
};

/** Exporting markers (the user's goal): the file opens in GPS tools — GPX
 *  carries `<wpt>` waypoints with name/desc/sym, GeoJSON a FeatureCollection
 *  of Points in lng-lat order with the shared title/description/icon props. */
describe('serializeMarkers', () => {
  it('emits GPX waypoints with name, description and symbol', () => {
    const { text, ext, mime } = serializeMarkers([camp], 'gpx');
    expect(ext).toBe('gpx');
    expect(mime).toBe('application/gpx+xml');
    expect(text).toContain('<wpt lat="69.6" lon="20">');
    expect(text).toContain('<name>Camp</name>');
    expect(text).toContain('<desc>by the river</desc>');
    expect(text).toContain('<sym>camping</sym>');
  });

  it('escapes XML in names and omits empty description', () => {
    const { text } = serializeMarkers([{ ...camp, name: 'A <B> & C', description: '' }], 'gpx');
    expect(text).toContain('<name>A &lt;B&gt; &amp; C</name>');
    expect(text).not.toContain('<desc>');
  });

  it('emits a GeoJSON FeatureCollection of points in lng-lat order', () => {
    const { text, ext } = serializeMarkers([camp], 'geojson');
    expect(ext).toBe('geojson');
    const parsed = JSON.parse(text);
    expect(parsed.type).toBe('FeatureCollection');
    expect(parsed.features[0].geometry).toEqual({ type: 'Point', coordinates: [20.0, 69.6] });
    expect(parsed.features[0].properties).toEqual({ title: 'Camp', description: 'by the river', icon: 'camping' });
  });
});
