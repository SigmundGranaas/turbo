import type { LatLng } from '../../geo';

/** Pure display rules for a route stop's label + its stable palette colour, and
 *  the drag-to-reorder target. Everything here is keyed on a coarse (~11 m) grid
 *  cell so a stop's name/colour follows the *coordinate*, not the list position —
 *  that is what makes them survive a reorder or a re-solve. Web-idiomatic mirror
 *  of Android's `StopLabels` / `StopPalette` (+ `dragReorderTarget`); no React,
 *  so the whole judgement is unit-tested. */

// ~11 m at Nordic latitudes: 0.0001° lat ≈ 11.1 m. Quantize onto this grid.
const GRID = 10_000;

function quantize(point: LatLng): { qLat: number; qLng: number } {
  return { qLat: Math.round(point.lat * GRID), qLng: Math.round(point.lng * GRID) };
}

/** Stable ~11 m grid-cell key for a point — the name-cache key + the colour seed.
 *  A string (not a packed 53-bit float) so it stays exact for any world coord. */
export function gridKey(point: LatLng): string {
  const { qLat, qLng } = quantize(point);
  return `${qLat},${qLng}`;
}

/** Trimmed plain-decimal coordinates, e.g. `69.9607, 23.2715` — the always-
 *  available fallback shown in the row's single fixed-height slot until (if ever)
 *  a name resolves. */
export function trimmedCoords(point: LatLng): string {
  return `${point.lat.toFixed(4)}, ${point.lng.toFixed(4)}`;
}

/** What the row renders in its single line: the cached name when there is one,
 *  otherwise the trimmed coordinates. Both occupy the same slot, so resolving a
 *  name is an in-place text swap — no reflow. */
export function label(cachedName: string | null | undefined, point: LatLng): string {
  const name = cachedName?.trim();
  return name && name.length > 0 ? name : trimmedCoords(point);
}

/** Distinct, map-legible hues for intermediate stops (the vias). Deliberately
 *  excludes the start-green / end-red role colours. */
const STOP_PALETTE = [
  '#1E88E5', // blue
  '#8E24AA', // purple
  '#F9A825', // amber
  '#00897B', // teal
  '#D81B60', // pink
  '#6D4C41', // brown
] as const;

/** Start green / end red role colours (design). */
export const START_GREEN = '#2E7D32';
export const END_RED = '#C0392B';

/** The via colour for a stop, chosen by its grid cell so the same place always
 *  draws the same colour — stable across a reorder because it is keyed on the
 *  coordinate, not the index. (Folds the quantized lat/lng exactly as Android's
 *  packed-key hash does.) */
export function stopPaletteColorOf(point: LatLng): string {
  const { qLat, qLng } = quantize(point);
  const idx = ((qLat ^ qLng) & 0x7fffffff) % STOP_PALETTE.length;
  return STOP_PALETTE[idx];
}

/** Per-stop colour: start green, end red; every via takes its stable palette
 *  colour keyed on the coordinate, so a stop keeps its colour even as its index
 *  changes. `last` is the destination index (waypoints.length - 1). */
export function stopColor(index: number, last: number, point: LatLng): string {
  if (index === 0) return START_GREEN;
  if (index === last) return END_RED;
  return stopPaletteColorOf(point);
}

/** Where a stop dragged from [from] by [dyPx] vertical pixels lands, given each
 *  row is [rowHeightPx] tall. Rounds to the nearest row and clamps into the list,
 *  so a drag can never target a slot that doesn't exist. The drag handle feeds it
 *  the live delta and commits the result once on release. */
export function dragReorderTarget(from: number, dyPx: number, rowHeightPx: number, count: number): number {
  if (count <= 1 || rowHeightPx <= 0) return from;
  const steps = Math.round(dyPx / rowHeightPx);
  return Math.max(0, Math.min(count - 1, from + steps));
}
