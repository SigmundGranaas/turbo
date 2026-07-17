import { useEffect, type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { LatLng } from '../geo';
import { measureAvailability } from '../map-core';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icon';
import { reverseGeocode } from '../features/markers';
import { getConditions } from '../api/conditions';

/** Screen position (CSS px) the card anchors at. */
export interface CardAnchor {
  x: number;
  y: number;
}

/**
 * The unified map-point card — the web mirror of Android's `MapLongPressMenu`,
 * driven by the `reduceMapPointCard` seam. A tappable mini-weather header (place
 * + temp → full forecast) above the point actions: an expandable Add Marker row
 * (revealing Marker / Photo / Weather Pin), Route Here, and Measure (disabled
 * with a hint while offline). Dismisses on Escape or an action.
 */
export function MapPointCard({
  dark,
  point,
  anchor,
  expanded,
  online,
  onToggleAddMarker,
  onMarker,
  onPhoto,
  onWeatherPin,
  onRouteHere,
  onMeasure,
  onForecast,
  onClose,
}: {
  dark: boolean;
  point: LatLng;
  anchor: CardAnchor;
  expanded: boolean;
  online: boolean;
  onToggleAddMarker: () => void;
  onMarker: () => void;
  onPhoto: () => void;
  onWeatherPin: () => void;
  onRouteHere: () => void;
  onMeasure: () => void;
  onForecast: (name: string) => void;
  onClose: () => void;
}) {
  const { lat, lng } = point;

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
  const temp = wxQ.data ? `${Math.round(wxQ.data.now.tempC)}°` : wxQ.isLoading ? '…' : '—';
  const measure = measureAvailability(online);

  // Dismiss on Escape only — an outside map click is handled by the host's tap
  // handler (which drives the reducer), so outside dismissal lives in one place.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  // Anchor near the press, but keep the whole card on-screen. Errs tall so the
  // expanded sub-row never slips off the bottom.
  const W = 256;
  const H = expanded ? 420 : 300;
  const left = Math.max(8, Math.min(window.innerWidth - W - 8, anchor.x));
  const top = Math.max(8, Math.min(window.innerHeight - H - 8, anchor.y));

  return (
    <div className="tm-pop" style={{ position: 'absolute', left, top, width: W, zIndex: 30 }}>
      <Glass dark={dark} level="panel" radius={20} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 2 }}>
        {/* mini-weather header → full forecast */}
        <button
          className="tm-btn"
          onClick={() => onForecast(placeLabel)}
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

        {/* Add Marker — primary, expands to Marker / Photo / Weather Pin */}
        <ActionRow
          icon="add_location_alt"
          label="Add marker"
          primary
          trailing={<Icon name={expanded ? 'expand_less' : 'expand_more'} size={20} color="var(--on-secondary-container)" />}
          onClick={onToggleAddMarker}
        />
        {expanded && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2, paddingLeft: 16 }}>
            <ActionRow icon="place" label="Marker" onClick={onMarker} />
            <ActionRow icon="add_a_photo" label="Photo" onClick={onPhoto} />
            <ActionRow icon="cloud" label="Weather pin" onClick={onWeatherPin} />
          </div>
        )}

        <ActionRow icon="navigation" label="Route here" onClick={onRouteHere} />

        {/* Measure — gated on connectivity: online enables it, offline disables
            it with a hint (mirrors Android's offline Measure state). */}
        <ActionRow
          icon="straighten"
          label="Measure"
          hint={measure.hint}
          disabled={!measure.enabled}
          onClick={onMeasure}
        />
      </Glass>
    </div>
  );
}

/** One card action as a full-width row (icon-in-circle + label). Disabled rows
 *  dim and show an optional hint instead of firing. */
function ActionRow({
  icon,
  label,
  hint,
  primary,
  disabled,
  trailing,
  onClick,
}: {
  icon: string;
  label: string;
  hint?: string;
  primary?: boolean;
  disabled?: boolean;
  trailing?: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className="tm-btn"
      disabled={disabled}
      title={hint}
      onClick={disabled ? undefined : onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        width: '100%',
        padding: '9px 12px',
        borderRadius: 14,
        border: 'none',
        cursor: disabled ? 'not-allowed' : 'pointer',
        textAlign: 'left',
        opacity: disabled ? 0.5 : 1,
        background: primary ? 'var(--secondary-container)' : 'transparent',
        color: primary ? 'var(--on-secondary-container)' : 'var(--on-surface)',
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
          flexShrink: 0,
          background: 'color-mix(in srgb, var(--primary) 14%, transparent)',
        }}
      >
        <Icon name={icon} size={19} color="var(--primary)" weight={500} fill />
      </span>
      <span style={{ flex: 1, minWidth: 0 }}>
        <span style={{ display: 'block', font: '500 15px/19px var(--font-sans)' }}>{label}</span>
        {hint && <span style={{ display: 'block', font: '400 12px/15px var(--font-sans)', color: 'var(--on-surface-variant)' }}>{hint}</span>}
      </span>
      {trailing}
    </button>
  );
}
