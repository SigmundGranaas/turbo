import { useEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from 'react';
import { useRouting } from './routingStore';
import { ROUTE_PROFILES } from './api';
import { stopColor, label, dragReorderTarget } from './stops';
import { stopNames } from './stopNames';
import { SidePanel, Eyebrow, StatTile, Tabs } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { useUiStore } from '../../store/uiStore';
import { formatDistance, formatElev } from '../../format';
import type { LatLng } from '../../geo';

const hm = (s: number) => {
  const t = Math.round(s / 60);
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
};

const ROW_GAP = 8;

/** The routing tool side panel — profile tabs, the ordered stops list (each stop
 *  colour-coded and drag-reorderable, its name lazily reverse-geocoded), a
 *  round-trip toggle, and live stats that fill in as the SSE solver streams.
 *  Ports the Phase-4 `RoutePlanner` (the Route Styles / preset picker was removed
 *  — the default preset stays wired in the store). Visibility + "save route as a
 *  track" are the host's job, passed via `onClose`/`onSaveAsTrack`. */
export function RoutePlannerPanel({
  dark,
  onClose,
  onSaveAsTrack,
  saving,
}: {
  dark: boolean;
  onClose: () => void;
  onSaveAsTrack: () => void;
  saving: boolean;
}) {
  const r = useRouting();
  const units = useUiStore((s) => s.units);
  const profileIdx = Math.max(0, ROUTE_PROFILES.findIndex((p) => p.key === r.profile));
  const last = r.waypoints.length - 1;

  return (
    <SidePanel
      dark={dark}
      title="Plan a route"
      onClose={onClose}
      footer={
        <div style={{ display: 'flex', gap: 10 }}>
          <Btn label="Clear" tone="surface" full onClick={r.clear} />
          <Btn label={saving ? 'Saving…' : 'Save route'} icon="bookmark_add" full onClick={r.plan && !saving ? onSaveAsTrack : undefined} />
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
          <div style={{ display: 'flex', flexDirection: 'column', gap: ROW_GAP }}>
            {r.waypoints.map((w, i) => (
              <StopRow
                key={i}
                point={w}
                index={i}
                last={last}
                count={r.waypoints.length}
                onRemove={() => r.removeWaypoint(i)}
                onMove={(from, to) => r.moveWaypoint(from, to)}
              />
            ))}
          </div>
        )}

        <RoundTripToggle on={r.roundTrip} disabled={r.waypoints.length < 2} onToggle={() => r.setRoundTrip(!r.roundTrip)} />

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

/** Lazily reverse-geocode a stop's name once (cached per ~11 m grid cell) and
 *  re-render in place when it lands. Until then the row shows trimmed coords, so
 *  the coords→name transition is a same-slot text swap — no reflow. */
function useStopName(point: LatLng): string | undefined {
  const [name, setName] = useState<string | undefined>(() => stopNames.cached(point));
  useEffect(() => {
    let live = true;
    setName(stopNames.cached(point));
    void stopNames.resolve(point).then((n) => {
      if (live && n) setName(n);
    });
    return () => {
      live = false;
    };
    // Resolve once per grid cell — the label follows the coordinate, not the index.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [point.lat, point.lng]);
  return name;
}

/** One stop: a colour-coded dot (matching its on-map ring), its role + lazily
 *  resolved name / coords, a drag handle that reorders on drop, accessible
 *  up/down reorder buttons (the drag's keyboard-reachable fallback), and remove. */
function StopRow({
  point,
  index,
  last,
  count,
  onRemove,
  onMove,
}: {
  point: LatLng;
  index: number;
  last: number;
  count: number;
  onRemove: () => void;
  onMove: (from: number, to: number) => void;
}) {
  const name = useStopName(point);
  const color = stopColor(index, last, point);
  const rowRef = useRef<HTMLDivElement>(null);
  const dragStartY = useRef<number | null>(null);
  const dragDy = useRef(0);
  const [dragging, setDragging] = useState(false);

  const role = index === 0 ? 'Start' : index === last && last > 0 ? 'Destination' : `Stop ${index}`;

  const onHandleDown = (e: ReactPointerEvent<HTMLButtonElement>) => {
    if (count <= 1) return;
    e.stopPropagation();
    dragStartY.current = e.clientY;
    dragDy.current = 0;
    setDragging(true);
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onHandleMove = (e: ReactPointerEvent<HTMLButtonElement>) => {
    if (dragStartY.current == null) return;
    dragDy.current = e.clientY - dragStartY.current;
  };
  const onHandleUp = (e: ReactPointerEvent<HTMLButtonElement>) => {
    if (dragStartY.current == null) return;
    try { e.currentTarget.releasePointerCapture(e.pointerId); } catch { /* released */ }
    const rowH = (rowRef.current?.offsetHeight ?? 56) + ROW_GAP;
    const target = dragReorderTarget(index, dragDy.current, rowH, count);
    dragStartY.current = null;
    setDragging(false);
    if (target !== index) onMove(index, target);
  };

  return (
    <div ref={rowRef} style={{ display: 'flex', alignItems: 'center', gap: 12, opacity: dragging ? 0.6 : 1 }}>
      <button
        aria-label="Drag to reorder stop"
        title="Drag to reorder"
        onPointerDown={onHandleDown}
        onPointerMove={onHandleMove}
        onPointerUp={onHandleUp}
        onPointerCancel={onHandleUp}
        style={{
          border: 'none',
          background: 'transparent',
          cursor: count > 1 ? 'grab' : 'default',
          color: 'var(--on-surface-variant)',
          display: 'flex',
          padding: 0,
          touchAction: 'none',
          flexShrink: 0,
        }}
      >
        <Icon name="drag_indicator" size={20} color="var(--on-surface-variant)" />
      </button>
      <div
        style={{
          width: 36,
          height: 36,
          borderRadius: 12,
          background: 'var(--surface-container-high)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexShrink: 0,
        }}
      >
        <Icon name={index === 0 ? 'trip_origin' : index === last ? 'flag' : 'place'} size={18} color={color} fill weight={500} />
      </div>
      <div
        style={{
          flex: 1,
          minWidth: 0,
          background: 'var(--surface-container-high)',
          borderRadius: 12,
          padding: '10px 14px',
          font: '500 14px/18px var(--font-sans)',
          color: 'var(--on-surface)',
        }}
      >
        {role}
        <span
          style={{
            color: 'var(--on-surface-variant)',
            fontWeight: 400,
            display: 'block',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
        >
          {label(name, point)}
        </span>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
        <button
          aria-label="Move stop up"
          onClick={() => index > 0 && onMove(index, index - 1)}
          disabled={index === 0}
          style={reorderBtn(index === 0)}
        >
          <Icon name="keyboard_arrow_up" size={16} color="var(--on-surface-variant)" />
        </button>
        <button
          aria-label="Move stop down"
          onClick={() => index < last && onMove(index, index + 1)}
          disabled={index === last}
          style={reorderBtn(index === last)}
        >
          <Icon name="keyboard_arrow_down" size={16} color="var(--on-surface-variant)" />
        </button>
      </div>
      <button
        onClick={onRemove}
        title="Remove"
        aria-label="Remove stop"
        style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', display: 'flex', flexShrink: 0 }}
      >
        <Icon name="close" size={18} color="var(--on-surface-variant)" />
      </button>
    </div>
  );
}

const reorderBtn = (disabled: boolean): CSSProperties => ({
  border: 'none',
  background: 'transparent',
  cursor: disabled ? 'default' : 'pointer',
  opacity: disabled ? 0.3 : 1,
  color: 'var(--on-surface-variant)',
  display: 'flex',
  padding: 0,
});

/** Round-trip toggle: solve a self-avoiding loop back to the start (sends the
 *  `round_trip` flag to the backend). Disabled until there are two points. */
function RoundTripToggle({ on, disabled, onToggle }: { on: boolean; disabled: boolean; onToggle: () => void }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label="Round trip"
      onClick={disabled ? undefined : onToggle}
      disabled={disabled}
      style={{
        marginTop: 18,
        width: '100%',
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        border: 'none',
        background: 'var(--surface-container-high)',
        borderRadius: 12,
        padding: '12px 14px',
        cursor: disabled ? 'default' : 'pointer',
        opacity: disabled ? 0.5 : 1,
        textAlign: 'left',
      }}
    >
      <Icon name="loop" size={20} color="var(--on-surface-variant)" />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ font: '500 14px/18px var(--font-sans)', color: 'var(--on-surface)' }}>Round trip</div>
        <div style={{ font: '400 12px/16px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Loop back to the start, avoiding the way out</div>
      </div>
      <span
        aria-hidden
        style={{
          width: 40,
          height: 24,
          borderRadius: 12,
          background: on ? 'var(--primary)' : 'var(--surface-container-highest)',
          border: on ? 'none' : '2px solid var(--outline)',
          position: 'relative',
          flexShrink: 0,
          transition: 'background 120ms',
        }}
      >
        <span
          style={{
            position: 'absolute',
            top: on ? 4 : 5,
            left: on ? 20 : 5,
            width: on ? 16 : 12,
            height: on ? 16 : 12,
            borderRadius: '50%',
            background: on ? 'var(--on-primary)' : 'var(--outline)',
            transition: 'left 120ms, top 120ms, width 120ms, height 120ms',
          }}
        />
      </span>
    </button>
  );
}
