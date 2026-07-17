import { describe, it, expect } from 'vitest';
import type { LatLng } from '../../geo';
import {
  dragReorderTarget,
  gridKey,
  label,
  START_GREEN,
  END_RED,
  stopColor,
  stopPaletteColorOf,
  trimmedCoords,
} from './stops';

const at = (lat: number, lng: number): LatLng => ({ lat, lng });

/** A stop's colour and name are pinned to its coordinate, not its position in the
 *  list — so reordering the stops never re-colours or re-labels them. This is the
 *  whole point of keying on the ~11 m grid cell. */
describe('stop identity follows the coordinate', () => {
  it('two points in the same ~11 m cell share a grid key (and one just outside does not)', () => {
    expect(gridKey(at(69.9607, 23.2715))).toBe(gridKey(at(69.96072, 23.27152)));
    expect(gridKey(at(69.9607, 23.2715))).not.toBe(gridKey(at(69.9612, 23.2721)));
  });

  it('a via keeps its palette colour after the list is reordered', () => {
    const via = at(69.9702, 23.31);
    // colour is a function of the coordinate alone
    const before = stopPaletteColorOf(via);
    // ...even when that same via now sits at a different index in a re-ordered list
    expect(stopPaletteColorOf(via)).toBe(before);
    // distinct coordinates generally land on distinct hues
    expect(stopPaletteColorOf(at(60.1, 10.1))).not.toBe(stopPaletteColorOf(at(63.4, 10.4)));
  });

  it('stopColor is start-green at the head, end-red at the tail, and a via hue between', () => {
    const first = at(69.96, 23.27);
    const mid = at(69.97, 23.29);
    const last = at(69.98, 23.31);
    const lastIdx = 2;
    expect(stopColor(0, lastIdx, first)).toBe(START_GREEN);
    expect(stopColor(2, lastIdx, last)).toBe(END_RED);
    const midColor = stopColor(1, lastIdx, mid);
    expect(midColor).not.toBe(START_GREEN);
    expect(midColor).not.toBe(END_RED);
    // the via colour is exactly its coordinate's palette colour
    expect(midColor).toBe(stopPaletteColorOf(mid));
  });

  it('the middle stop that was second still gets START_GREEN once it moves to the front', () => {
    const p = at(69.97, 23.29);
    // role colour is decided by position, so a reorder that makes it the start turns it green
    expect(stopColor(1, 2, p)).toBe(stopPaletteColorOf(p));
    expect(stopColor(0, 2, p)).toBe(START_GREEN);
  });
});

/** The row shows a name when one is known, otherwise trimmed coordinates — the
 *  in-place fallback that lets the coords→name swap happen without reflow. */
describe('stop label', () => {
  it('shows trimmed 4-decimal coordinates when there is no name', () => {
    expect(trimmedCoords(at(69.96071, 23.27154))).toBe('69.9607, 23.2715');
    expect(label(undefined, at(69.96071, 23.27154))).toBe('69.9607, 23.2715');
  });

  it('shows the cached name once one resolves', () => {
    expect(label('Storgata 1', at(69.96, 23.27))).toBe('Storgata 1');
  });

  it('falls back to coordinates for a blank / whitespace name', () => {
    expect(label('   ', at(69.96, 23.27))).toBe(trimmedCoords(at(69.96, 23.27)));
  });
});

/** The pure drag target: a stop dragged by N row-heights lands N slots away,
 *  rounded and clamped into the list. */
describe('dragReorderTarget', () => {
  const row = 56;

  it('dragging down one row moves the stop one slot later', () => {
    expect(dragReorderTarget(2, row, row, 5)).toBe(3);
  });

  it('dragging up two rows moves two slots earlier', () => {
    expect(dragReorderTarget(3, -2 * row, row, 5)).toBe(1);
  });

  it('a half-row nudge rounds to the nearest slot', () => {
    expect(dragReorderTarget(2, row * 0.4, row, 5)).toBe(2); // stays
    expect(dragReorderTarget(2, row * 0.6, row, 5)).toBe(3); // tips over
  });

  it('the target clamps to the ends of the list', () => {
    expect(dragReorderTarget(1, -10 * row, row, 4)).toBe(0);
    expect(dragReorderTarget(1, 10 * row, row, 4)).toBe(3);
  });

  it('degenerate inputs are no-ops (single item, zero row height)', () => {
    expect(dragReorderTarget(2, 500, row, 1)).toBe(2);
    expect(dragReorderTarget(2, 500, 0, 5)).toBe(2);
  });
});
