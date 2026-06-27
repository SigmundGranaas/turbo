import { useRef, type PointerEvent as ReactPointerEvent } from 'react';
import { useProjectedLayer, viewportDpr } from './useProjectedLayer';
import { useMapEngine } from './MapEngineContext';
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
 *  (the design's `RouteLine`) — terrain-aware, so the line + stops drape on the
 *  3D relief. Pointer-transparent by default so it never blocks the map.
 *
 *  When `onWaypointDrag` is supplied the stop rings become draggable handles:
 *  dragging one re-places that waypoint via the terrain-aware `unproject_ground`
 *  (so it lands on the surface the user sees in 3D); the new position is
 *  committed on release. */
export function RouteOverlay({
  coords,
  waypoints,
  dashed,
  color = 'var(--primary)',
  onWaypointDrag,
}: {
  coords: LatLng[];
  waypoints: LatLng[];
  dashed?: boolean;
  /** Line + waypoint stroke colour (defaults to the theme primary). */
  color?: string;
  /** When set, waypoint rings are draggable; called with the final position on
   *  release (one call per drag, so the solver re-runs once). */
  onWaypointDrag?: (index: number, p: LatLng) => void;
}) {
  const engine = useMapEngine();
  const haloRef = useRef<SVGPathElement>(null);
  const lineRef = useRef<SVGPathElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const stopRefs = useRef<(SVGCircleElement | null)[]>([]);
  const coordsRef = useRef(coords);
  const wpRef = useRef(waypoints);
  coordsRef.current = downsample(coords);
  wpRef.current = waypoints;

  // The ring currently being dragged — the per-frame projection skips it so the
  // manual (pointer-driven) position isn't overwritten. The latest ground point
  // is stashed and committed on release.
  const dragIdx = useRef<number | null>(null);
  const dragLatLng = useRef<LatLng | null>(null);

  useProjectedLayer((eng, dpr) => {
    let d = '';
    const pts = coordsRef.current;
    for (let i = 0; i < pts.length; i++) {
      const p = eng.project(pts[i].lat, pts[i].lng);
      if (p) d += `${d ? 'L' : 'M'}${(p[0] / dpr).toFixed(1)} ${(p[1] / dpr).toFixed(1)}`;
    }
    haloRef.current?.setAttribute('d', d);
    lineRef.current?.setAttribute('d', d);
    wpRef.current.forEach((w, i) => {
      const c = stopRefs.current[i];
      if (!c || i === dragIdx.current) return; // leave the dragged ring under the finger
      const p = eng.project(w.lat, w.lng);
      if (p) {
        c.setAttribute('cx', String(p[0] / dpr));
        c.setAttribute('cy', String(p[1] / dpr));
        c.style.display = 'block';
      } else {
        c.style.display = 'none';
      }
    });
  });

  const onHandleDown = (i: number) => (e: ReactPointerEvent<SVGCircleElement>) => {
    if (!onWaypointDrag) return;
    e.stopPropagation();
    dragIdx.current = i;
    dragLatLng.current = null;
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onHandleMove = (e: ReactPointerEvent<SVGCircleElement>) => {
    if (dragIdx.current == null) return;
    e.stopPropagation();
    const rect = svgRef.current?.getBoundingClientRect();
    const x = e.clientX - (rect?.left ?? 0);
    const y = e.clientY - (rect?.top ?? 0);
    // Follow the finger immediately (CSS px == SVG user units, no viewBox).
    e.currentTarget.setAttribute('cx', String(x));
    e.currentTarget.setAttribute('cy', String(y));
    // Resolve the ground point under the finger (terrain-aware in 3D).
    const g = engine?.unproject_ground(x * viewportDpr(), y * viewportDpr());
    if (g && g.length >= 2) dragLatLng.current = { lat: g[0], lng: g[1] };
  };
  const onHandleUp = (i: number) => (e: ReactPointerEvent<SVGCircleElement>) => {
    if (dragIdx.current == null) return;
    e.stopPropagation();
    try { e.currentTarget.releasePointerCapture(e.pointerId); } catch { /* released */ }
    const p = dragLatLng.current;
    dragIdx.current = null;
    dragLatLng.current = null;
    if (p) onWaypointDrag?.(i, p);
  };

  return (
    <svg ref={svgRef} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 5 }}>
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
          r={onWaypointDrag ? 9 : 8}
          fill="var(--surface)"
          stroke={color}
          strokeWidth={3.5}
          onPointerDown={onWaypointDrag ? onHandleDown(i) : undefined}
          onPointerMove={onWaypointDrag ? onHandleMove : undefined}
          onPointerUp={onWaypointDrag ? onHandleUp(i) : undefined}
          onPointerCancel={onWaypointDrag ? onHandleUp(i) : undefined}
          style={onWaypointDrag ? { pointerEvents: 'auto', cursor: 'grab', touchAction: 'none' } : undefined}
        />
      ))}
    </svg>
  );
}
