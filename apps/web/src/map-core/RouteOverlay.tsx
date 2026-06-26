import { useRef } from 'react';
import { useProjectedLayer } from './useProjectedLayer';
import type { LatLng } from '../geo';

// Cap projected vertices per frame so a dense route stays cheap to reproject.
const MAX_VERTS = 400;

function downsample(coords: LatLng[]): LatLng[] {
  if (coords.length <= MAX_VERTS) return coords;
  const step = Math.ceil(coords.length / MAX_VERTS);
  const out: LatLng[] = [];
  for (let i = 0; i < coords.length; i += step) out.push(coords[i]);
  if (out[out.length - 1] !== coords[coords.length - 1]) out.push(coords[coords.length - 1]);
  return out;
}

/** The planned/preview route drawn as an SVG polyline over the map, with
 *  waypoint stop rings. Vertices are projected every frame via `map.project`
 *  (the design's `RouteLine`). Pointer-transparent so it never blocks the map. */
export function RouteOverlay({
  coords,
  waypoints,
  dashed,
  color = 'var(--primary)',
}: {
  coords: LatLng[];
  waypoints: LatLng[];
  dashed?: boolean;
  /** Line + waypoint stroke colour (defaults to the theme primary). */
  color?: string;
}) {
  const haloRef = useRef<SVGPathElement>(null);
  const lineRef = useRef<SVGPathElement>(null);
  const stopRefs = useRef<(SVGCircleElement | null)[]>([]);
  const coordsRef = useRef(coords);
  const wpRef = useRef(waypoints);
  coordsRef.current = downsample(coords);
  wpRef.current = waypoints;

  useProjectedLayer((engine, dpr) => {
    let d = '';
    const pts = coordsRef.current;
    for (let i = 0; i < pts.length; i++) {
      const p = engine.project(pts[i].lat, pts[i].lng);
      if (p) d += `${d ? 'L' : 'M'}${(p[0] / dpr).toFixed(1)} ${(p[1] / dpr).toFixed(1)}`;
    }
    haloRef.current?.setAttribute('d', d);
    lineRef.current?.setAttribute('d', d);
    wpRef.current.forEach((w, i) => {
      const c = stopRefs.current[i];
      if (!c) return;
      const p = engine.project(w.lat, w.lng);
      if (p) {
        c.setAttribute('cx', String(p[0] / dpr));
        c.setAttribute('cy', String(p[1] / dpr));
        c.style.display = 'block';
      } else {
        c.style.display = 'none';
      }
    });
  });

  return (
    <svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 5 }}>
      <path ref={haloRef} fill="none" stroke="rgba(255,255,255,.7)" strokeWidth={9} strokeLinejoin="round" strokeLinecap="round" />
      <path
        ref={lineRef}
        fill="none"
        stroke={color}
        strokeWidth={5}
        strokeLinejoin="round"
        strokeLinecap="round"
        strokeDasharray={dashed ? '2 10' : undefined}
      />
      {waypoints.map((_, i) => (
        <circle
          key={i}
          ref={(el) => {
            stopRefs.current[i] = el;
          }}
          r={8}
          fill="var(--surface)"
          stroke={color}
          strokeWidth={3.5}
        />
      ))}
    </svg>
  );
}
