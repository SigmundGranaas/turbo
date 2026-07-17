import { useEffect, useRef } from 'react';
import { Glass, GlassIconBtn, Divider } from './Glass';
import { compassVisible } from './compass';

/** Bottom-right floating control cluster over the map — reduced (Phase 1) to the
 *  five essentials: Compass (reset-north), Layers, Location, and a zoom group.
 *  The 3D + Sun controls moved into the layers sheet as sliders (the hard
 *  decoupling: neither may move the camera). Handlers are wired by the host. */
export interface MapRailState {
  layers?: boolean;
  following?: boolean;
  /** Compass "Lock rotation" — pins the bearing so gestures can't rotate. */
  rotationLocked?: boolean;
}
export interface MapRailHandlers {
  onLayers?: () => void;
  onRecenter?: () => void;
  onCompass?: () => void;
  /** Toggle the persisted rotation lock (compass long-press / right-click). */
  onToggleRotationLock?: () => void;
  onZoomIn?: () => void;
  onZoomOut?: () => void;
}

export function MapRail({
  dark,
  state,
  on,
  getBearing,
}: {
  dark: boolean;
  state: MapRailState;
  on: MapRailHandlers;
  /** Live camera bearing accessor (deg). The compass needle tracks it every
   *  frame so it points to true north as the map rotates. */
  getBearing?: () => number;
}) {
  const { layers, following, rotationLocked } = state;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, alignItems: 'flex-end' }}>
      <Glass dark={dark} radius={22} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <CompassBtn
          getBearing={getBearing}
          active={following}
          locked={rotationLocked}
          onClick={on.onCompass}
          onToggleLock={on.onToggleRotationLock}
        />
      </Glass>
      <Glass dark={dark} radius={22} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <GlassIconBtn icon="layers" active={layers} title="Map layers" onClick={on.onLayers} />
        <Divider inset />
        <GlassIconBtn
          icon={following ? 'my_location' : 'near_me'}
          active={following}
          fill={following}
          title="Recenter"
          onClick={on.onRecenter}
        />
      </Glass>
      <Glass dark={dark} radius={28} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 2 }}>
        <GlassIconBtn icon="add" iconSize={22} title="Zoom in" onClick={on.onZoomIn} />
        <Divider inset />
        <GlassIconBtn icon="remove" iconSize={22} title="Zoom out" onClick={on.onZoomOut} />
      </Glass>
    </div>
  );
}

function CompassBtn({
  getBearing,
  active,
  locked,
  onClick,
  onToggleLock,
}: {
  getBearing?: () => number;
  active?: boolean;
  locked?: boolean;
  onClick?: () => void;
  onToggleLock?: () => void;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const holdRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const heldRef = useRef(false);
  // Rotate the needle to point at true north as the camera bearing changes, and
  // auto-hide the whole button when the map is within ~0.5° of north (spec's
  // compassVisible rule) — unless rotation is LOCKED, where we keep it visible so
  // the lock can be released. Driven off rAF + mutated directly (no React
  // re-render per frame, mirrors how the marker layer projects).
  useEffect(() => {
    if (!getBearing) return;
    let raf = 0;
    let last = NaN;
    const tick = () => {
      const b = getBearing() || 0;
      // `last` starts NaN, so seed on the first frame (NaN comparisons are false).
      if (svgRef.current && (Number.isNaN(last) || Math.abs(b - last) > 0.2)) {
        svgRef.current.style.transform = `rotate(${-b}deg)`;
        last = b;
      }
      if (btnRef.current) btnRef.current.style.display = compassVisible(b) || locked ? '' : 'none';
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [getBearing, locked]);

  // Long-press (touch) or right-click toggles the rotation lock; a plain tap
  // resets north.
  const startHold = () => {
    heldRef.current = false;
    holdRef.current = setTimeout(() => {
      heldRef.current = true;
      onToggleLock?.();
    }, 450);
  };
  const endHold = () => {
    if (holdRef.current) clearTimeout(holdRef.current);
    holdRef.current = null;
  };
  return (
    <button
      ref={btnRef}
      className="tm-icon-btn"
      title={locked ? 'Rotation locked · long-press to unlock, tap to reset north' : 'Compass · tap to reset north, long-press to lock rotation'}
      onClick={() => {
        if (heldRef.current) return; // the long-press already toggled the lock
        onClick?.();
      }}
      onPointerDown={startHold}
      onPointerUp={endHold}
      onPointerLeave={endHold}
      onContextMenu={(e) => {
        e.preventDefault();
        onToggleLock?.();
      }}
      style={{
        position: 'relative',
        width: 48,
        height: 48,
        borderRadius: 18,
        cursor: 'pointer',
        border: 'none',
        background: locked ? 'var(--secondary-container)' : active ? 'var(--tertiary-container)' : 'transparent',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <svg ref={svgRef} width="26" height="26" viewBox="0 0 35 34">
        <path fill={active ? 'var(--on-tertiary-container)' : 'var(--error)'} d="M17.4 4 L20.6 16 H14.2 Z" />
        <path fill={active ? 'var(--on-tertiary-container)' : 'var(--primary)'} d="M17.4 30 L14.2 18 H20.6 Z" opacity=".55" />
      </svg>
      {locked && (
        <span
          className="material-symbols-outlined"
          style={{
            position: 'absolute',
            right: 3,
            bottom: 2,
            fontSize: 13,
            color: 'var(--on-secondary-container)',
          }}
        >
          lock
        </span>
      )}
    </button>
  );
}

/** Coordinate + elevation + scale readout, bottom-left over the map. */
export function MapReadout({
  coord = '60.39° N · 5.32° E',
  elev = '— m',
  scale = '500 m',
}: {
  coord?: string;
  elev?: string;
  scale?: string;
}) {
  const txt = 'rgba(35,25,23,.82)';
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        font: '500 11px/1 var(--font-sans)',
        color: txt,
        textShadow: '0 1px 2px rgba(255,255,255,.7)',
      }}
    >
      <span style={{ fontVariantNumeric: 'tabular-nums' }}>{coord}</span>
      <span style={{ opacity: 0.4 }}>|</span>
      <span>{elev}</span>
      <span style={{ opacity: 0.4 }}>|</span>
      <div style={{ display: 'flex', alignItems: 'flex-end', gap: 6 }}>
        <div style={{ width: 46, height: 6, borderBottom: `2px solid ${txt}`, borderLeft: `2px solid ${txt}`, borderRight: `2px solid ${txt}` }} />
        <span>{scale}</span>
      </div>
    </div>
  );
}
