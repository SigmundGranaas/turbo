import { useRouting } from '../../store/routingStore';
import { usePaths } from '../../store/pathsStore';
import { ROUTE_PRESETS, ROUTE_PROFILES } from '../../api/routing';
import { useCreateTrack } from '../paths/useTracks';
import { SidePanel, Eyebrow, StatTile, Chip2, Tabs } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { useUiStore } from '../../store/uiStore';
import { formatDistance, formatElev } from '../../format';

const hm = (s: number) => {
  const t = Math.round(s / 60);
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
};

/** The routing tool side panel — profile tabs, the ordered stops list, preset
 *  chips, and live stats that fill in as the SSE solver streams. Ports the
 *  design's `RoutePlanner` (minus follow/navigation, out of scope on web). */
export function RoutePlannerPanel({ dark }: { dark: boolean }) {
  const r = useRouting();
  const units = useUiStore((s) => s.units);
  const createTrack = useCreateTrack();
  const profileIdx = Math.max(0, ROUTE_PROFILES.findIndex((p) => p.key === r.profile));
  const profileIcon = ROUTE_PROFILES[profileIdx].icon;

  const saveAsTrack = () => {
    if (!r.plan) return;
    createTrack.mutate(
      {
        name: `Route · ${new Date().toLocaleDateString()}`,
        points: r.plan.coords,
        iconKey: profileIcon,
        distanceM: r.plan.distanceM,
        ascentM: r.plan.ascentM,
        movingTimeS: Math.round(r.plan.durationS),
      },
      {
        onSuccess: (t) => {
          r.close();
          usePaths.getState().openDetail(t.id);
        },
      },
    );
  };
  const stopLabel = (i: number) => (i === 0 ? 'Start' : i === r.waypoints.length - 1 && r.waypoints.length > 1 ? 'Destination' : `Stop ${i}`);

  return (
    <SidePanel
      dark={dark}
      title="Plan a route"
      onClose={r.close}
      footer={
        <div style={{ display: 'flex', gap: 10 }}>
          <Btn label="Clear" tone="surface" full onClick={r.clear} />
          <Btn label={createTrack.isPending ? 'Saving…' : 'Save route'} icon="bookmark_add" full onClick={r.plan && !createTrack.isPending ? saveAsTrack : undefined} />
        </div>
      }
    >
      <div style={{ paddingTop: 8, paddingBottom: 8 }}>
        <Tabs items={ROUTE_PROFILES.map((p) => ({ label: p.label, icon: p.icon }))} active={profileIdx} onPick={(i) => r.setProfile(ROUTE_PROFILES[i].key)} />

        <Eyebrow style={{ margin: '20px 0 10px' }}>Stops</Eyebrow>
        {r.waypoints.length === 0 ? (
          <div style={{ font: '400 13px/19px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
            Click the map to drop your start, then each stop. Click a marker to route to it.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {r.waypoints.map((w, i) => (
              <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <div
                  style={{
                    width: 36,
                    height: 36,
                    borderRadius: 12,
                    background: 'var(--surface-container-high)',
                    color: 'var(--primary)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    flexShrink: 0,
                  }}
                >
                  <Icon name={i === 0 ? 'trip_origin' : 'place'} size={18} color="var(--primary)" fill weight={500} />
                </div>
                <div
                  style={{
                    flex: 1,
                    background: 'var(--surface-container-high)',
                    borderRadius: 12,
                    padding: '10px 14px',
                    font: '500 14px/18px var(--font-sans)',
                    color: 'var(--on-surface)',
                  }}
                >
                  {stopLabel(i)}
                  <span style={{ color: 'var(--on-surface-variant)', fontWeight: 400 }}>
                    {' '}· {w.lat.toFixed(4)}, {w.lng.toFixed(4)}
                  </span>
                </div>
                <button
                  onClick={() => r.removeWaypoint(i)}
                  title="Remove"
                  style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', display: 'flex' }}
                >
                  <Icon name="close" size={18} color="var(--on-surface-variant)" />
                </button>
              </div>
            ))}
          </div>
        )}

        <Eyebrow style={{ margin: '20px 0 10px' }}>Preset</Eyebrow>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          {ROUTE_PRESETS.map((p) => (
            <Chip2 key={p.key} label={p.label} icon={p.icon} selected={r.preset === p.key} onClick={() => r.setPreset(p.key)} />
          ))}
        </div>

        <Eyebrow style={{ margin: '20px 0 10px' }}>Route</Eyebrow>
        {r.status === 'solving' && (
          <div style={{ font: '500 14px/20px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Solving route…</div>
        )}
        {r.status === 'error' && (
          <div style={{ font: '400 14px/20px var(--font-sans)', color: 'var(--error)' }}>{r.error ?? 'Routing failed.'}</div>
        )}
        {r.status === 'idle' && r.waypoints.length < 2 && (
          <div style={{ font: '400 13px/19px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Add at least two points to plan a route.</div>
        )}
        {r.plan && (
          <div style={{ display: 'flex', gap: 8 }}>
            <StatTile icon="straighten" value={formatDistance(r.plan.distanceM, units)} label="Distance" />
            <StatTile icon="trending_up" value={formatElev(r.plan.ascentM, units)} label="Ascent" />
            <StatTile icon="schedule" value={hm(r.plan.durationS)} label="Est. time" />
          </div>
        )}
      </div>
    </SidePanel>
  );
}
