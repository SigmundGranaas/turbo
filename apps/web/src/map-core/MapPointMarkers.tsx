import { useRef } from 'react';
import { useProjectedLayer } from './useProjectedLayer';
import { useMapPoints } from './mapPoints';

/** Kernel overlay for transient map points: the click indicator (a ring at the
 *  last-clicked location) and the 3D orbit anchor (a pin at the point the camera
 *  is orbiting around). Both are projected every frame via the terrain-aware
 *  `project()`, so they stay glued to the 3D relief — never the flat baseline.
 *  Reads its points from the `useMapPoints` store each frame (no React re-render
 *  on camera move). Pointer-transparent. */
export function MapPointMarkers() {
  const clickRef = useRef<HTMLDivElement>(null);
  const orbitRef = useRef<HTMLDivElement>(null);

  useProjectedLayer((engine, dpr) => {
    const { click, orbit } = useMapPoints.getState();
    const ce = clickRef.current;
    if (ce) {
      const p = click ? engine.project(click.lat, click.lng) : undefined;
      if (p) {
        ce.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -50%)`;
        ce.style.display = 'block';
      } else {
        ce.style.display = 'none';
      }
    }
    const oe = orbitRef.current;
    if (oe) {
      const p = orbit ? engine.project(orbit.lat, orbit.lng) : undefined;
      if (p) {
        // Anchor the pin's tip on the ground point (translate -100% lifts it).
        oe.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -100%)`;
        oe.style.display = 'block';
      } else {
        oe.style.display = 'none';
      }
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
      {/* 3D orbit anchor — a pin whose tip rests on the terrain */}
      <div ref={orbitRef} data-testid="map-orbit-pin" style={{ position: 'absolute', left: 0, top: 0, display: 'none' }}>
        <svg width={26} height={34} viewBox="0 0 26 34" style={{ filter: 'drop-shadow(0 2px 3px rgba(0,0,0,.4))' }}>
          <path
            d="M13 33C13 33 24 19.5 24 12A11 11 0 1 0 2 12C2 19.5 13 33 13 33Z"
            fill="var(--primary)"
            stroke="#fff"
            strokeWidth={2}
          />
          <circle cx={13} cy={12} r={4} fill="#fff" />
        </svg>
      </div>
    </div>
  );
}
