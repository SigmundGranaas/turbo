import type { Track } from '../../api/tracks';
import { kindForIcon } from '../../activities/kinds';
import { usePaths } from '../../store/pathsStore';
import { SidePanel, Tabs } from '../../ui/Panel';
import { Cookie } from '../../ui/Cookie';
import { Icon } from '../../ui/Icon';
import { useUiStore } from '../../store/uiStore';
import { formatDistance, formatElev } from '../../format';

/** The "Saved" panel — the user's tracks as a tappable list. (Collections +
 *  markers tabs land with their features; this is the paths list.) */
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
  return (
    <SidePanel dark={dark} title="Saved" subtitle={`${tracks.length} path${tracks.length === 1 ? '' : 's'}`} onClose={onClose}>
      <div style={{ paddingTop: 8, paddingBottom: 12 }}>
        <div style={{ marginBottom: 12 }}>
          <Tabs items={[{ label: 'Paths' }, { label: 'Collections' }]} active={0} onPick={(i) => i === 1 && usePaths.getState().setTab('collections')} />
        </div>
        {loading && <div style={{ font: '400 14px/20px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Loading…</div>}
        {!loading && tracks.length === 0 && (
          <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)', paddingTop: 8 }}>
            No saved paths yet. Plan a route and save it, and it’ll show up here.
          </div>
        )}
        {tracks.map((t) => {
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
                <Icon name={kind.icon} size={22} color="var(--primary)" fill weight={500} />
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
