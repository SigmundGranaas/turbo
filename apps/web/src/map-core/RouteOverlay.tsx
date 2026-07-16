import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from 'react';
import { useProjectedLayer, viewportDpr } from './useProjectedLayer';
import { useMapEngine } from './MapEngineContext';
import { setMapLine } from './mapContent';
import type { LatLng } from '../geo';

/** The planned/preview route (or a selected track). The LINE is map content:
 *  it's published to the content plane (`setMapLine`) and the engine draws it
 *  as a scene-declared `line` layer, draped on the 3D relief (plan P6.3).
 *  This component keeps only the interactive chrome: the waypoint stop rings,
 *  which stay DOM/SVG because they are drag handles (pointer capture, cursor,
 *  live drag feedback) — positioned per frame via the projection hook.
 *
 *  When `onWaypointDrag` is supplied the stop rings become draggable handles:
 *  dragging one re-places that waypoint via the terrain-aware `unproject_ground`
 *  (so it lands on the surface the user sees in 3D); the new position is
 *  committed on release. */
export function RouteOverlay({
  coords,
  waypoints,
  dashed,
  dash,
  color = 'var(--primary)',
  contentKey = 'route',
  onWaypointDrag,
}: {
  coords: LatLng[];
  waypoints: LatLng[];
  dashed?: boolean;
  /** Explicit dash pattern (a track's user-chosen line style); wins over [dashed]. */
  dash?: number[];
  /** Line + waypoint stroke colour (defaults to the theme primary). */
  color?: string;
  /** Content-plane key — lets independent owners (route planner, selected
   *  track) each publish one line without clobbering the other. */
  contentKey?: string;
  /** When set, waypoint rings are draggable; called with the final position on
   *  release (one call per drag, so the solver re-runs once). */
  onWaypointDrag?: (index: number, p: LatLng) => void;
}) {
  const engine = useMapEngine();
  const svgRef = useRef<SVGSVGElement>(null);
  const stopRefs = useRef<(SVGCircleElement | null)[]>([]);
  const wpRef = useRef(waypoints);
  wpRef.current = waypoints;

  // Publish the line to the content plane; clear it on unmount/empty.
  useEffect(() => {
    setMapLine(contentKey, coords.length >= 2 ? { coords, color, dashed, dash } : null);
  }, [coords, color, dashed, dash, contentKey]);
  useEffect(() => () => setMapLine(contentKey, null), [contentKey]);

  // The ring currently being dragged — the per-frame projection skips it so the
  // manual (pointer-driven) position isn't overwritten. The latest ground point
  // is stashed and committed on release.
  const dragIdx = useRef<number | null>(null);
  const dragLatLng = useRef<LatLng | null>(null);

  useProjectedLayer((eng, dpr) => {
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
