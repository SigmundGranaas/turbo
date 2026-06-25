import { useEffect, useRef, useState, type RefObject } from 'react';
import type { TurboMap } from 'turbomap-web';

const DPR = () => Math.min(window.devicePixelRatio || 1, 2);

/** The blue "you are here" dot. Watches the browser geolocation and projects
 *  the fix onto the map every frame (like the marker layer), so it stays glued
 *  to the ground as the camera pans/zooms/rotates/tilts. Pointer-transparent. */
export function UserLocation({ mapRef }: { mapRef: RefObject<TurboMap | null> }) {
  const dotRef = useRef<HTMLDivElement>(null);
  const fix = useRef<{ lat: number; lng: number } | null>(null);
  const [hasFix, setHasFix] = useState(false);

  // Continuously track position (high-accuracy). watchPosition keeps the dot
  // live as the user moves; we only store the latest fix and let rAF place it.
  useEffect(() => {
    if (!('geolocation' in navigator)) return;
    const id = navigator.geolocation.watchPosition(
      (pos) => {
        fix.current = { lat: pos.coords.latitude, lng: pos.coords.longitude };
        setHasFix(true);
      },
      () => {},
      { enableHighAccuracy: true, maximumAge: 5000, timeout: 15000 },
    );
    return () => navigator.geolocation.clearWatch(id);
  }, []);

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const m = mapRef.current;
      const el = dotRef.current;
      if (m && el && fix.current) {
        const p = m.project(fix.current.lat, fix.current.lng);
        if (p) {
          const dpr = DPR();
          el.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -50%)`;
          el.style.display = 'block';
        } else {
          el.style.display = 'none';
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [mapRef]);

  if (!hasFix) return null;
  return (
    <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 5 }}>
      <div ref={dotRef} style={{ position: 'absolute', left: 0, top: 0, display: 'none' }}>
        {/* soft accuracy halo + crisp dot with a white ring, like Google/Apple */}
        <div
          style={{
            position: 'absolute',
            left: '50%',
            top: '50%',
            width: 44,
            height: 44,
            transform: 'translate(-50%, -50%)',
            borderRadius: '50%',
            background: 'rgba(37, 99, 235, 0.18)',
          }}
        />
        <div
          style={{
            position: 'absolute',
            left: '50%',
            top: '50%',
            width: 16,
            height: 16,
            transform: 'translate(-50%, -50%)',
            borderRadius: '50%',
            background: '#2563eb',
            border: '3px solid #fff',
            boxShadow: '0 1px 4px rgba(0,0,0,.4)',
          }}
        />
      </div>
    </div>
  );
}
