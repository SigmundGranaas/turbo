import { useState } from 'react';
import { type ExportFormat, type Track, serializeTrack } from '../../api/tracks';
import { kindForIcon } from '../../activities/kinds';
import { SidePanel, Eyebrow, StatTile } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { Cookie } from '../../ui/Cookie';
import { ElevationChart } from '../../ui/ElevationChart';
import { useUiStore } from '../../store/uiStore';
import { formatDistance, formatElev } from '../../format';

const hm = (s?: number) => (s ? `${Math.floor(s / 3600)}:${String(Math.round((s % 3600) / 60)).padStart(2, '0')}` : '—');

function downloadTrack(t: Track, fmt: ExportFormat) {
  const { text, ext, mime } = serializeTrack(t, fmt);
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${(t.name || 'track').replace(/[^\w-]+/g, '_')}.${ext}`;
  a.click();
  URL.revokeObjectURL(url);
}

/** Saved-path detail — stats, elevation profile, show-on-map + export GPX,
 *  delete. Ports the design's `PathDetail`. */
export function PathDetailPanel({
  dark,
  track,
  onShow,
  onEdit,
  onSave,
  onShare,
  onDelete,
  onBack,
  onClose,
}: {
  dark: boolean;
  track: Track;
  onShow: () => void;
  onEdit: () => void;
  onSave: () => void;
  onShare: () => void;
  onDelete: () => void;
  onBack: () => void;
  onClose: () => void;
}) {
  const units = useUiStore((s) => s.units);
  const [exportOpen, setExportOpen] = useState(false);
  const km = (m: number) => formatDistance(m, units);
  const kind = kindForIcon(track.iconKey);
  const high = track.elevations?.length ? Math.max(...track.elevations) : null;
  const recorded = track.recordedAt ? new Date(track.recordedAt).toLocaleDateString() : null;

  return (
    <SidePanel
      dark={dark}
      title={track.name || 'Untitled path'}
      subtitle={[recorded, kind.label].filter(Boolean).join(' · ')}
      onClose={onClose}
      headerExtra={
        <div style={{ display: 'flex', gap: 8, flexShrink: 0 }}>
          <button onClick={onShare} title="Share" style={{ width: 38, height: 38, borderRadius: 9999, border: 'none', cursor: 'pointer', background: 'var(--surface-container-highest)', color: 'var(--on-surface-variant)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <Icon name="ios_share" size={20} color="var(--on-surface-variant)" />
          </button>
          <button onClick={onSave} title="Add to collection" style={{ width: 38, height: 38, borderRadius: 9999, border: 'none', cursor: 'pointer', background: 'var(--surface-container-highest)', color: 'var(--on-surface-variant)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <Icon name="bookmark_add" size={20} color="var(--on-surface-variant)" />
          </button>
          <button onClick={onBack} title="Back to list" style={{ width: 38, height: 38, borderRadius: 9999, border: 'none', cursor: 'pointer', background: 'var(--surface-container-highest)', color: 'var(--on-surface-variant)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <Icon name="arrow_back" size={20} color="var(--on-surface-variant)" />
          </button>
        </div>
      }
    >
      <div style={{ paddingTop: 4 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 4 }}>
          <Cookie size={48} lobes={8} fill="var(--primary-container)">
            <Icon name={kind.icon} size={24} color="var(--on-primary-container)" fill weight={500} />
          </Cookie>
          <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
            {km(track.distanceM)} · {track.points.length} points
          </div>
        </div>

        <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
          <StatTile value={km(track.distanceM)} label="Distance" />
          <StatTile value={track.ascentM != null ? formatElev(track.ascentM, units) : '—'} label="Ascent" />
          <StatTile value={hm(track.movingTimeS)} label="Moving" />
          <StatTile value={high != null ? formatElev(high, units) : '—'} label="High pt" />
        </div>

        {track.elevations && track.elevations.length >= 2 && (
          <>
            <Eyebrow style={{ margin: '22px 0 8px' }}>Elevation profile</Eyebrow>
            <div style={{ background: 'var(--surface-container-low)', borderRadius: 18, padding: 12 }}>
              <ElevationChart points={track.elevations} />
            </div>
          </>
        )}

        <div style={{ display: 'flex', gap: 10, margin: '20px 0' }}>
          <Btn label="Show on map" icon="my_location" full onClick={onShow} />
          <Btn label="Edit" icon="edit" tone="surface" full onClick={onEdit} />
        </div>

        <div style={{ marginBottom: 12 }}>
          <Btn label="Export" icon="download" trailingIcon={exportOpen ? 'expand_less' : 'expand_more'} tone="surface" full onClick={() => setExportOpen((v) => !v)} />
          {exportOpen && (
            <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
              {(['gpx', 'geojson', 'kml'] as const).map((fmt) => (
                <Btn key={fmt} label={fmt.toUpperCase()} size="sm" tone="tonal" full onClick={() => { downloadTrack(track, fmt); setExportOpen(false); }} />
              ))}
            </div>
          )}
        </div>

        <div style={{ marginBottom: 16 }}>
          <Btn label="Delete path" icon="delete" tone="surface" full onClick={onDelete} />
        </div>
      </div>
    </SidePanel>
  );
}
