import { useId } from 'react';

/** Height-vs-distance area chart for a track's elevation series. Ported from the
 *  design's `ElevationChart`. */
export function ElevationChart({ points, w = 304, h = 104 }: { points: number[]; w?: number; h?: number }) {
  const id = useId().replace(/:/g, '');
  const pts = points.length >= 2 ? points : [0, 0];
  const max = Math.max(...pts);
  const min = Math.min(...pts);
  const nx = (i: number) => (i / (pts.length - 1)) * w;
  const ny = (v: number) => h - 8 - ((v - min) / (max - min || 1)) * (h - 20);
  const line = pts.map((v, i) => `${i === 0 ? 'M' : 'L'}${nx(i).toFixed(1)} ${ny(v).toFixed(1)}`).join(' ');
  const area = `${line} L${w} ${h} L0 ${h} Z`;
  return (
    <svg width="100%" height={h} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ display: 'block' }}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="var(--primary)" stopOpacity="0.3" />
          <stop offset="1" stopColor="var(--primary)" stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={area} fill={`url(#${id})`} />
      <path d={line} fill="none" stroke="var(--primary)" strokeWidth="2.4" strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}
