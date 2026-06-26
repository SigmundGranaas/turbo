import { useRef } from 'react';
import { useProjectedLayer } from '../../map-core';
import type { Marker } from './api';
import { kindForIcon } from '../../activities/kinds';

/** Billboarded marker pins overlaid on the map. Positioned every frame via the
 *  shared `useProjectedLayer` (`engine.project(lat,lng)`) — the design renders
 *  entities as screen-space billboards over the 3D terrain. The layer is
 *  pointer-transparent except the pins, so it never blocks map pan. */
export function MarkerPins({
  markers,
  selectedId,
  onSelect,
}: {
  markers: Marker[];
  selectedId?: string;
  onSelect: (id: string) => void;
}) {
  const layerRef = useRef<HTMLDivElement>(null);

  useProjectedLayer((engine, dpr) => {
    const layer = layerRef.current;
    if (!layer) return;
    for (const node of Array.from(layer.children)) {
      const el = node as HTMLElement;
      const lat = Number(el.dataset.lat);
      const lng = Number(el.dataset.lng);
      const p = engine.project(lat, lng);
      if (p) {
        el.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -100%)`;
        el.style.display = 'block';
      } else {
        el.style.display = 'none';
      }
    }
  });

  return (
    <div ref={layerRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 6 }}>
      {markers.map((mk) => {
        const kind = kindForIcon(mk.icon);
        const selected = mk.id === selectedId;
        return (
          <button
            key={mk.id}
            data-lat={mk.lat}
            data-lng={mk.lng}
            title={mk.name}
            onClick={() => onSelect(mk.id)}
            style={{
              position: 'absolute',
              left: 0,
              top: 0,
              pointerEvents: 'auto',
              border: 'none',
              background: 'transparent',
              padding: 0,
              cursor: 'pointer',
              filter: 'drop-shadow(0 4px 4px rgba(40,20,12,.35))',
            }}
          >
            <div
              style={{
                width: selected ? 44 : 36,
                height: selected ? 44 : 36,
                background: kind.color,
                borderRadius: '50% 50% 50% 3px',
                transform: 'rotate(45deg)',
                border: `3px solid var(--surface)`,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                transition: 'width .15s, height .15s',
              }}
            >
              <div style={{ transform: 'rotate(-45deg)' }}>
                <span
                  className="material-symbols-outlined"
                  style={{ fontSize: selected ? 22 : 18, color: '#fff', fontVariationSettings: "'FILL' 1, 'wght' 600" }}
                >
                  {kind.icon}
                </span>
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
