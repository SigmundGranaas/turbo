import { describe, it, expect } from 'vitest';
import { parseTrack, trackStats, needsElevationBackfill, mergeElevations } from './trackImport';

const GPX = `<?xml version="1.0"?>
<gpx version="1.1"><trk><name>QA Imported Track</name><trkseg>
<trkpt lat="60.39" lon="5.32"><ele>10</ele></trkpt>
<trkpt lat="60.40" lon="5.33"><ele>55</ele></trkpt>
<trkpt lat="60.41" lon="5.35"><ele>120</ele></trkpt>
<trkpt lat="60.42" lon="5.34"><ele>80</ele></trkpt>
</trkseg></trk></gpx>`;

const KML = `<?xml version="1.0"?>
<kml><Document><Placemark><LineString><coordinates>
5.32,60.39,10 5.33,60.40,55 5.35,60.41,120
</coordinates></LineString></Placemark></Document></kml>`;

const GEOJSON = JSON.stringify({
  type: 'Feature',
  geometry: { type: 'LineString', coordinates: [[5.32, 60.39], [5.33, 60.40], [5.35, 60.41]] },
});

/** Importing a track file (the user's goal): the points come back in order with
 *  the right coordinates, regardless of GPX/KML/GeoJSON; too-short or junk files
 *  are rejected rather than producing a broken track. */
describe('parseTrack', () => {
  it('reads a GPX track: name, ordered points, and elevations', () => {
    const t = parseTrack(GPX)!;
    expect(t).not.toBeNull();
    expect(t.name).toBe('QA Imported Track');
    expect(t.points).toHaveLength(4);
    expect(t.points[0]).toEqual({ lat: 60.39, lng: 5.32 });
    expect(t.elevations).toEqual([10, 55, 120, 80]);
  });

  it('reads a KML LineString (lon,lat,ele coordinate tuples)', () => {
    const t = parseTrack(KML)!;
    expect(t.points).toHaveLength(3);
    expect(t.points[1]).toEqual({ lat: 60.4, lng: 5.33 });
  });

  it('reads a GeoJSON LineString (no elevations → nulls)', () => {
    const t = parseTrack(GEOJSON)!;
    expect(t.points).toHaveLength(3);
    expect(t.points[2]).toEqual({ lat: 60.41, lng: 5.35 });
  });

  it('rejects a file with fewer than two points', () => {
    const onePoint = `<gpx><trk><trkseg><trkpt lat="60.39" lon="5.32"/></trkseg></trk></gpx>`;
    expect(parseTrack(onePoint)).toBeNull();
  });

  it('rejects junk that is not a track', () => {
    expect(parseTrack('not a track at all')).toBeNull();
  });
});

/** Stats shown on the track (distance + climb): distance accumulates along the
 *  points; ascent/descent sum the up/down elevation deltas, and are absent when
 *  the file had no elevation data. */
describe('trackStats', () => {
  it('sums distance and ascent/descent from elevation deltas', () => {
    const t = parseTrack(GPX)!;
    const s = trackStats(t.points, t.elevations);
    expect(s.distanceM).toBeGreaterThan(0);
    // up: (55-10)+(120-55)=110 ; down: (120-80)=40
    expect(s.ascentM).toBeCloseTo(110, 5);
    expect(s.descentM).toBeCloseTo(40, 5);
  });

  it('omits elevation totals when there is no elevation data', () => {
    const s = trackStats([{ lat: 60.39, lng: 5.32 }, { lat: 60.4, lng: 5.33 }], [null, null]);
    expect(s.distanceM).toBeGreaterThan(0);
    expect(s.ascentM).toBeUndefined();
  });

  it('ignores metre-scale GPS jitter but keeps a slow steady climb (mirrors Android)', () => {
    const pts = Array.from({ length: 7 }, (_, i) => ({ lat: 60.39 + i * 0.001, lng: 5.32 }));
    // ±1.5 m oscillation around 100 m: pure noise, no real climb.
    const jitter = trackStats(pts, [100, 101.5, 99, 100.5, 99.5, 101, 100]);
    expect(jitter.ascentM).toBeCloseTo(0, 5);
    expect(jitter.descentM).toBeCloseTo(0, 5);
    // 100 → 112 m in 2 m steps: below the band per fix, but the reference
    // ratchets so the full height still commits.
    const climb = trackStats(pts, [100, 102, 104, 106, 108, 110, 112]);
    expect(climb.ascentM).toBeCloseTo(12, 5);
    expect(climb.descentM).toBeCloseTo(0, 5);
  });
});

/** DEM elevation backfill on import (the user's goal: a working elevation chart
 *  for GeoJSON/route files that carry no <ele>): tracks missing most elevation
 *  qualify; a file that brought its own data is left alone; merging prefers the
 *  file's value per vertex and falls back to the DEM sample. */
describe('elevation backfill decisions', () => {
  it('qualifies a track with no elevation data', () => {
    expect(needsElevationBackfill([null, null, null], 3)).toBe(true);
  });

  it('leaves a mostly-complete file alone', () => {
    expect(needsElevationBackfill([10, 20, null, 30], 4)).toBe(false);
  });

  it('skips degenerate and oversized tracks', () => {
    expect(needsElevationBackfill([null], 1)).toBe(false);
    expect(needsElevationBackfill(Array(5000).fill(null), 5000)).toBe(false);
  });

  it('merges per vertex: file value wins, sample fills, null where neither knows', () => {
    expect(mergeElevations([12, null, null], [100, 200, null], 3)).toEqual([12, 200, null]);
  });
});
