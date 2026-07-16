import { useMemo, useRef, useState } from 'react';
import type { Track } from './api';
import { kindForIcon } from '../../activities/kinds';
import { usePaths } from '../../store/pathsStore';
import { SidePanel, Tabs, FilledField } from '../../ui/Panel';
import { Cookie } from '../../ui/Cookie';
import { Icon } from '../../ui/Icon';
import { useUiStore } from '../../store/uiStore';
import { formatDistance, formatElev } from '../../format';
import { useCreateTrack } from './useTracks';
import { parseTrack, trackStats, needsElevationBackfill, mergeElevations } from './trackImport';
import { sampleElevations } from '../../api/elevation';

type SortKey = 'newest' | 'name' | 'longest';
const SORTS: { key: SortKey; label: string }[] = [
  { key: 'newest', label: 'Newest' },
  { key: 'name', label: 'Name' },
  { key: 'longest', label: 'Longest' },
];

/** Sort + filter the tracks (pure) — ports Android `sortAndFilterPaths`. */
function sortAndFilter(tracks: Track[], q: string, sort: SortKey): Track[] {
  const needle = q.trim().toLowerCase();
  const filtered = needle ? tracks.filter((t) => (t.name || '').toLowerCase().includes(needle)) : tracks.slice();
  return filtered.sort((a, b) => {
    if (sort === 'name') return (a.name || '').localeCompare(b.name || '');
    if (sort === 'longest') return b.distanceM - a.distanceM;
    // newest: recordedAt desc (fall back to keeping order)
    return (b.recordedAt ?? '').localeCompare(a.recordedAt ?? '');
  });
}

/** The "Saved" panel — the user's tracks as a searchable, sortable list, with
 *  GPX/KML/GeoJSON import. (Collections is the sibling tab.) */
export function PathsListPanel({
  dark,
  tracks,
  loading,
  onSelect,
  onClose,
}: {
  dark: boolean;
  tracks: Track[];
  loading: boolean;
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  const units = useUiStore((s) => s.units);
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<SortKey>('newest');
  const [importErr, setImportErr] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const create = useCreateTrack();
  const shown = useMemo(() => sortAndFilter(tracks, query, sort), [tracks, query, sort]);

  const onFile = async (file: File) => {
    setImportErr(null);
    try {
      const text = await file.text();
      const parsed = parseTrack(text);
      if (!parsed) {
        setImportErr('Couldn’t read a track from that file (need GPX, KML or GeoJSON with ≥ 2 points).');
        return;
      }
      // Files without per-point elevation get theirs from the tileserver DEM
      // (best-effort — a sampling outage just imports the track without them).
      let elevations = parsed.elevations;
      if (needsElevationBackfill(elevations, parsed.points.length)) {
        try {
          elevations = mergeElevations(elevations, await sampleElevations(parsed.points), parsed.points.length);
        } catch {
          /* keep the file's own (possibly empty) elevations */
        }
      }
      const stats = trackStats(parsed.points, elevations);
      const hasEle = elevations.some((e) => e != null);
      const name = parsed.name?.trim() || file.name.replace(/\.[^.]+$/, '');
      const created = await create.mutateAsync({
        name,
        points: parsed.points,
        elevations: hasEle ? elevations.map((e) => e ?? 0) : undefined,
        distanceM: stats.distanceM,
        ascentM: stats.ascentM,
        descentM: stats.descentM,
      });
      onSelect(created.id);
    } catch {
      setImportErr('Import failed — sign in to save imported tracks to your account.');
    }
  };

  return (
    <SidePanel
      dark={dark}
      title="Saved"
      subtitle={`${tracks.length} path${tracks.length === 1 ? '' : 's'}`}
      onClose={onClose}
      headerExtra={
        <button
          onClick={() => fileRef.current?.click()}
          title="Import GPX / KML / GeoJSON"
          style={{ width: 38, height: 38, borderRadius: 9999, border: 'none', cursor: 'pointer', background: 'var(--surface-container-highest)', color: 'var(--on-surface-variant)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}
        >
          <Icon name="upload_file" size={20} color="var(--on-surface-variant)" />
        </button>
      }
    >
      <input
        ref={fileRef}
        type="file"
        accept=".gpx,.kml,.geojson,.json,.xml,application/gpx+xml,application/vnd.google-earth.kml+xml,application/geo+json"
        style={{ display: 'none' }}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void onFile(f);
          e.target.value = '';
        }}
      />
      <div style={{ paddingTop: 8, paddingBottom: 12 }}>
        <div style={{ marginBottom: 12 }}>
          <Tabs items={[{ label: 'Paths' }, { label: 'Collections' }]} active={0} onPick={(i) => i === 1 && usePaths.getState().setTab('collections')} />
        </div>

        <FilledField label="Search paths" value={query} onChange={setQuery} placeholder="Search paths" />
        <div style={{ display: 'flex', gap: 8, margin: '12px 0 4px' }}>
          {SORTS.map((s) => {
            const sel = s.key === sort;
            return (
              <button
                key={s.key}
                onClick={() => setSort(s.key)}
                style={{
                  padding: '6px 14px',
                  borderRadius: 9999,
                  border: 'none',
                  cursor: 'pointer',
                  font: '500 13px/1 var(--font-sans)',
                  background: sel ? 'var(--secondary-container)' : 'var(--surface-container-high)',
                  color: sel ? 'var(--on-secondary-container)' : 'var(--on-surface-variant)',
                }}
              >
                {s.label}
              </button>
            );
          })}
        </div>

        {importErr && (
          <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--error)', padding: '8px 0' }}>{importErr}</div>
        )}
        {create.isPending && (
          <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--on-surface-variant)', padding: '8px 0' }}>Importing…</div>
        )}

        {loading && <div style={{ font: '400 14px/20px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Loading…</div>}
        {!loading && tracks.length === 0 && (
          <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)', paddingTop: 8 }}>
            No saved paths yet. Plan a route and save it, or import a GPX file, and it’ll show up here.
          </div>
        )}
        {!loading && tracks.length > 0 && shown.length === 0 && (
          <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)', paddingTop: 8 }}>
            No paths match “{query}”.
          </div>
        )}
        {shown.map((t) => {
          const kind = kindForIcon(t.iconKey);
          return (
            <button
              key={t.id}
              onClick={() => onSelect(t.id)}
              style={{
                width: '100%',
                display: 'flex',
                alignItems: 'center',
                gap: 14,
                padding: '10px 4px',
                border: 'none',
                background: 'transparent',
                cursor: 'pointer',
                textAlign: 'left',
              }}
            >
              <Cookie size={44} lobes={7} fill="var(--surface-container-high)">
                <Icon name={kind.icon} size={22} color={t.colorHex || 'var(--primary)'} fill weight={500} />
              </Cookie>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ font: '500 15px/20px var(--font-sans)', color: 'var(--on-surface)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {t.name || 'Untitled path'}
                </div>
                <div style={{ font: '400 13px/17px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
                  {formatDistance(t.distanceM, units)}
                  {t.ascentM != null ? ` · ${formatElev(t.ascentM, units)} ↑` : ''}
                </div>
              </div>
              <Icon name="chevron_right" size={20} color="var(--on-surface-variant)" />
            </button>
          );
        })}
      </div>
    </SidePanel>
  );
}
