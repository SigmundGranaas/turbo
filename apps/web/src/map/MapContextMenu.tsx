import { useEffect, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icon';
import { reverseGeocode } from '../features/markers';
import { getConditions } from '../api/conditions';

export interface ContextMenuTarget {
  /** Screen position (CSS px) where the menu anchors. */
  x: number;
  y: number;
  /** Geographic point under the press. */
  lat: number;
  lng: number;
}

interface Action {
  key: string;
  label: string;
  icon: string;
  primary?: boolean;
  run: () => void;
}

/** A long-press / click contextual menu over the map — mirrors the native
 *  Android `MapLongPressMenu`: a tappable mini-weather header (place + temp →
 *  full forecast) above a list of point actions. Dismisses on outside-press,
 *  Escape, or after an action. */
export function MapContextMenu({
  dark,
  target,
  onNewMarker,
  onRouteHere,
  onStartRoute,
  onForecast,
  onClose,
}: {
  dark: boolean;
  target: ContextMenuTarget;
  onNewMarker: () => void;
  onRouteHere: () => void;
  onStartRoute: () => void;
  onForecast: (name: string) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const { lat, lng } = target;

  const nameQ = useQuery({
    queryKey: ['ctx-name', lat.toFixed(4), lng.toFixed(4)],
    queryFn: () => reverseGeocode(lat, lng),
    staleTime: 300_000,
  });
  const wxQ = useQuery({
    queryKey: ['ctx-wx', lat.toFixed(3), lng.toFixed(3)],
    queryFn: () => getConditions(lat, lng),
    staleTime: 300_000,
    retry: false,
  });

  const coordLabel = `${lat.toFixed(4)}, ${lng.toFixed(4)}`;
  const placeLabel = nameQ.data || coordLabel;

  // Dismiss on outside pointer-down (which also covers a map pan starting) + Esc.
  useEffect(() => {
    const onDoc = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    // defer so the opening press itself doesn't immediately close it
    const t = setTimeout(() => {
      document.addEventListener('pointerdown', onDoc, true);
      document.addEventListener('keydown', onKey);
    }, 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('pointerdown', onDoc, true);
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  const act = (fn: () => void) => () => {
    fn();
    onClose();
  };

  const actions: Action[] = [
    { key: 'marker', label: 'New marker', icon: 'add_location_alt', primary: true, run: act(onNewMarker) },
    { key: 'route', label: 'Route here', icon: 'navigation', run: act(onRouteHere) },
    { key: 'start', label: 'Start route here', icon: 'trip_origin', run: act(onStartRoute) },
  ];

  // Anchor near the press, but keep the whole card on-screen.
  const W = 248;
  const H = 230;
  const left = Math.max(8, Math.min(window.innerWidth - W - 8, target.x));
  const top = Math.max(8, Math.min(window.innerHeight - H - 8, target.y));
  const temp = wxQ.data ? `${Math.round(wxQ.data.now.tempC)}°` : wxQ.isLoading ? '…' : '—';

  return (
    <div ref={ref} className="tm-pop" style={{ position: 'absolute', left, top, width: W, zIndex: 30 }}>
      <Glass dark={dark} level="panel" radius={20} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 2 }}>
        {/* mini-weather header → full forecast */}
        <button
          className="tm-btn"
          onClick={act(() => onForecast(placeLabel))}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            padding: '10px 12px',
            borderRadius: 14,
            border: 'none',
            cursor: 'pointer',
            textAlign: 'left',
            background: 'var(--surface-container-high)',
            color: 'var(--on-surface)',
          }}
        >
          <Icon name="device_thermostat" size={22} color="var(--primary)" weight={500} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ font: '700 17px/20px var(--font-sans)' }}>{temp}</div>
            <div
              style={{
                font: '400 12px/15px var(--font-sans)',
                color: 'var(--on-surface-variant)',
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
              }}
            >
              {placeLabel}
            </div>
          </div>
          <Icon name="chevron_right" size={20} color="var(--on-surface-variant)" />
        </button>

        {actions.map((a) => (
          <button
            key={a.key}
            className="tm-btn"
            onClick={a.run}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 12,
              width: '100%',
              padding: '9px 12px',
              borderRadius: 14,
              border: 'none',
              cursor: 'pointer',
              textAlign: 'left',
              background: a.primary ? 'var(--secondary-container)' : 'transparent',
              color: a.primary ? 'var(--on-secondary-container)' : 'var(--on-surface)',
            }}
          >
            <span
              style={{
                width: 32,
                height: 32,
                borderRadius: '50%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: 'color-mix(in srgb, var(--primary) 14%, transparent)',
              }}
            >
              <Icon name={a.icon} size={19} color="var(--primary)" weight={500} fill />
            </span>
            <span style={{ font: '500 15px/19px var(--font-sans)' }}>{a.label}</span>
          </button>
        ))}
      </Glass>
    </div>
  );
}
