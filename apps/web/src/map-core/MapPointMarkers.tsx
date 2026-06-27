import { useRef } from 'react';
import { useProjectedLayer } from './useProjectedLayer';
import { useMapPoints } from './mapPoints';

/** Kernel overlay for the click indicator — a ring at the last-clicked location,
 *  projected every frame via the terrain-aware `project()` so it stays glued to
 *  the 3D relief (never the flat baseline). Reads from the `useMapPoints` store
 *  each frame (no React re-render on camera move). Pointer-transparent. */
export function MapPointMarkers() {
  const clickRef = useRef<HTMLDivElement>(null);

  useProjectedLayer((engine, dpr) => {
    const { click } = useMapPoints.getState();
    const ce = clickRef.current;
    if (!ce) return;
    const p = click ? engine.project(click.lat, click.lng) : undefined;
    if (p) {
      ce.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -50%)`;
      ce.style.display = 'block';
    } else {
      ce.style.display = 'none';
    }
  });

  return (
    <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 6 }}>
      {/* click indicator — a target ring sitting on the ground */}
      <div ref={clickRef} data-testid="map-click-ring" style={{ position: 'absolute', left: 0, top: 0, display: 'none' }}>
        <div
          style={{
            width: 18,
            height: 18,
            borderRadius: '50%',
            border: '2px solid var(--primary)',
            background: 'color-mix(in srgb, var(--primary) 22%, transparent)',
            boxShadow: '0 1px 4px rgba(0,0,0,.35)',
          }}
        />
      </div>
    </div>
  );
}
