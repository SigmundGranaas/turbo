import { useEffect, useRef, type RefObject } from 'react';
import type { TurboMap } from 'turbomap-web';
import type { LatLng } from '../../geo';

const DPR = () => Math.min(window.devicePixelRatio || 1, 2);
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
  mapRef,
  coords,
  waypoints,
  dashed,
}: {
  mapRef: RefObject<TurboMap | null>;
  coords: LatLng[];
  waypoints: LatLng[];
  dashed?: boolean;
}) {
  const haloRef = useRef<SVGPathElement>(null);
  const lineRef = useRef<SVGPathElement>(null);
  const stopRefs = useRef<(SVGCircleElement | null)[]>([]);
  const coordsRef = useRef(coords);
  const wpRef = useRef(waypoints);
  coordsRef.current = downsample(coords);
  wpRef.current = waypoints;

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const m = mapRef.current;
      if (m) {
        const dpr = DPR();
        let d = '';
        const pts = coordsRef.current;
        for (let i = 0; i < pts.length; i++) {
          const p = m.project(pts[i].lat, pts[i].lng);
          if (p) d += `${d ? 'L' : 'M'}${(p[0] / dpr).toFixed(1)} ${(p[1] / dpr).toFixed(1)}`;
        }
        haloRef.current?.setAttribute('d', d);
        lineRef.current?.setAttribute('d', d);
        wpRef.current.forEach((w, i) => {
          const c = stopRefs.current[i];
          if (!c) return;
          const p = m.project(w.lat, w.lng);
          if (p) {
            c.setAttribute('cx', String(p[0] / dpr));
            c.setAttribute('cy', String(p[1] / dpr));
            c.style.display = 'block';
          } else {
            c.style.display = 'none';
          }
        });
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [mapRef]);

  return (
    <svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 5 }}>
      <path ref={haloRef} fill="none" stroke="rgba(255,255,255,.7)" strokeWidth={9} strokeLinejoin="round" strokeLinecap="round" />
      <path
        ref={lineRef}
        fill="none"
        stroke="var(--primary)"
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
          stroke="var(--primary)"
          strokeWidth={3.5}
        />
      ))}
    </svg>
  );
}
