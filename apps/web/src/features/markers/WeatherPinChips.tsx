import { useRef } from 'react';
import { useProjectedLayer } from '../../map-core';
import type { Marker } from './api';
import { weatherPinUiState } from './weatherPin';

/** Compact emoji glyph for a met.no `symbol_code` — the same mapping the
 *  conditions panel uses, kept tiny here for the on-map chip. */
function symbolGlyph(code?: string): string {
  if (!code) return '·';
  const c = code.toLowerCase();
  if (c.includes('thunder')) return '⛈️';
  if (c.includes('sleet')) return '🌨️';
  if (c.includes('snow')) return '❄️';
  if (c.includes('rain')) return '🌧️';
  if (c.includes('fog')) return '🌫️';
  if (c.includes('cloudy') && !c.includes('partly')) return '☁️';
  if (c.includes('partlycloudy') || c.includes('fair')) return '⛅';
  if (c.includes('clearsky')) return '☀️';
  return '·';
}

/** On-map weather-pin glyphs (map-overhaul Phase 3): each weather-pin marker
 *  draws a live "temp + condition" chip from its cached forecast instead of a
 *  plain scene pin (Standard markers still render through `MarkerPins`). The
 *  chip is DOM chrome anchored to the ground point — projected every frame via
 *  the terrain-aware `project()` so it stays glued to the relief — and a tap
 *  opens the point's forecast detail. Renders "…" until a forecast is cached. */
export function WeatherPinChips({
  markers,
  onTap,
}: {
  markers: Marker[];
  onTap: (m: Marker) => void;
}) {
  const pins = markers.filter((m) => m.markerKind === 'WeatherPin');
  const refs = useRef(new Map<string, HTMLButtonElement | null>());
  const now = Date.now();

  useProjectedLayer((engine, dpr) => {
    for (const m of pins) {
      const el = refs.current.get(m.id);
      if (!el) continue;
      const p = engine.project(m.lat, m.lng);
      if (p) {
        // Anchor the chip's bottom tip at the ground point.
        el.style.transform = `translate(${p[0] / dpr}px, ${p[1] / dpr}px) translate(-50%, -100%)`;
        el.style.display = 'inline-flex';
      } else {
        el.style.display = 'none';
      }
    }
  });

  return (
    <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 6 }}>
      {pins.map((m) => {
        const ui = weatherPinUiState(m, now);
        return (
          <button
            key={m.id}
            ref={(el) => {
              refs.current.set(m.id, el);
            }}
            onClick={() => onTap(m)}
            title={m.name}
            data-testid="weather-pin-chip"
            style={{
              position: 'absolute',
              left: 0,
              top: 0,
              display: 'none',
              alignItems: 'center',
              gap: 5,
              padding: '5px 10px',
              borderRadius: 9999,
              border: 'none',
              cursor: 'pointer',
              pointerEvents: 'auto',
              font: '600 14px/1 var(--font-sans)',
              color: 'var(--on-surface)',
              background: 'var(--surface-container-high)',
              boxShadow: 'var(--elevation-2)',
              whiteSpace: 'nowrap',
            }}
          >
            {ui ? (
              <>
                <span style={{ fontSize: 15 }}>{symbolGlyph(ui.symbolCode)}</span>
                <span>{Math.round(ui.temperatureC)}°</span>
              </>
            ) : (
              <span style={{ color: 'var(--on-surface-variant)' }}>…</span>
            )}
          </button>
        );
      })}
    </div>
  );
}
