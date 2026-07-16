import { useState } from 'react';
import { serializeMarkers, type Marker, type MarkerExportFormat } from './api';
import { kindForIcon } from '../../activities/kinds';
import { SidePanel, Eyebrow, StatTile, GlassIconBtnSolid } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { useConditions } from '../../api/useConditions';
import { windDir } from '../../api/conditions';
import { useUiStore } from '../../store/uiStore';
import { formatTemp, formatWind } from '../../format';

const fmt = (lat: number, lng: number) =>
  `${Math.abs(lat).toFixed(4)}° ${lat >= 0 ? 'N' : 'S'}, ${Math.abs(lng).toFixed(4)}° ${lng >= 0 ? 'E' : 'W'}`;

/** Serialize the marker and trigger a browser download (mirrors `downloadTrack`). */
function downloadMarker(m: Marker, fmt2: MarkerExportFormat) {
  const { text, ext, mime } = serializeMarkers([m], fmt2);
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${(m.name || 'marker').replace(/[^\w-]+/g, '_')}.${ext}`;
  a.click();
  URL.revokeObjectURL(url);
}

/** Marker detail side panel. Ports the design's `MarkerDetail` — actions row,
 *  note, and a conditions strip (weather wiring lands with port doc 13). */
export function MarkerDetailPanel({
  dark,
  marker,
  onEdit,
  onDelete,
  onRoute,
  onSave,
  onShare,
  onConditions,
  onClose,
}: {
  dark: boolean;
  marker: Marker;
  onEdit: () => void;
  onDelete: () => void;
  onRoute: () => void;
  onSave: () => void;
  onShare: () => void;
  onConditions: () => void;
  onClose: () => void;
}) {
  const kind = kindForIcon(marker.icon);
  const [exportOpen, setExportOpen] = useState(false);
  const units = useUiStore((s) => s.units);
  const cond = useConditions(marker.lat, marker.lng);
  const w = cond.data?.now;
  const dash = cond.isLoading ? '…' : '—';
  return (
    <SidePanel dark={dark} title={marker.name || kind.label} subtitle={`${kind.label} · ${fmt(marker.lat, marker.lng)}`} onClose={onClose}>
      <div style={{ paddingTop: 4 }}>
        <div style={{ display: 'flex', gap: 10, marginTop: 4 }}>
          <Btn label="Route here" icon="navigation" full onClick={onRoute} />
          <GlassIconBtnSolid icon="bookmark_add" title="Save to collection" onClick={onSave} />
          <GlassIconBtnSolid icon="ios_share" title="Share" onClick={onShare} />
          <GlassIconBtnSolid icon="edit" title="Edit" onClick={onEdit} />
        </div>

        {marker.description && (
          <>
            <Eyebrow style={{ margin: '22px 0 8px' }}>Note</Eyebrow>
            <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)' }}>{marker.description}</div>
          </>
        )}

        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', margin: '22px 0 10px' }}>
          <Eyebrow>Conditions now</Eyebrow>
          <button onClick={onConditions} style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--primary)', font: '600 12px/1 var(--font-sans)' }}>
            Forecast →
          </button>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <StatTile icon="device_thermostat" value={w ? formatTemp(w.tempC, units) : dash} label="Temp" />
          <StatTile icon="air" value={w ? formatWind(w.windMs, units) : dash} label={w ? `Wind ${windDir(w.windDeg)}` : 'Wind'} />
          <StatTile icon="water_drop" value={w ? (w.precipMm ?? 0).toFixed(1) : dash} label="mm/h" />
        </div>

        <div style={{ marginTop: 24 }}>
          <Btn label="Export" icon="download" trailingIcon={exportOpen ? 'expand_less' : 'expand_more'} tone="surface" full onClick={() => setExportOpen((v) => !v)} />
          {exportOpen && (
            <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
              {(['gpx', 'geojson'] as const).map((fmt) => (
                <Btn key={fmt} label={fmt.toUpperCase()} size="sm" tone="tonal" full onClick={() => { downloadMarker(marker, fmt); setExportOpen(false); }} />
              ))}
            </div>
          )}
        </div>

        <div style={{ marginTop: 12, marginBottom: 16 }}>
          <Btn label="Delete marker" icon="delete" tone="surface" full onClick={onDelete} />
        </div>
      </div>
    </SidePanel>
  );
}
