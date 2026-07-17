import { useEffect, useRef } from 'react';
import { Glass, GlassIconBtn, Divider } from './Glass';

/** Bottom-right floating control cluster over the map — reduced (Phase 1) to the
 *  five essentials: Compass (reset-north), Layers, Location, and a zoom group.
 *  The 3D + Sun controls moved into the layers sheet as sliders (the hard
 *  decoupling: neither may move the camera). Handlers are wired by the host. */
export interface MapRailState {
  layers?: boolean;
  following?: boolean;
}
export interface MapRailHandlers {
  onLayers?: () => void;
  onRecenter?: () => void;
  onCompass?: () => void;
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
  const { layers, following } = state;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, alignItems: 'flex-end' }}>
      <Glass dark={dark} radius={22} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <CompassBtn getBearing={getBearing} active={following} onClick={on.onCompass} />
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

function CompassBtn({ getBearing, active, onClick }: { getBearing?: () => number; active?: boolean; onClick?: () => void }) {
  const svgRef = useRef<SVGSVGElement>(null);
  // Rotate the needle to point at true north as the camera bearing changes —
  // driven off rAF + mutated directly (no React re-render per frame, mirrors
  // how the marker layer projects).
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
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [getBearing]);
  return (
    <button className="tm-icon-btn"
      title="Compass · reset north"
      onClick={onClick}
      style={{
        width: 48,
        height: 48,
        borderRadius: 18,
        cursor: 'pointer',
        border: 'none',
        background: active ? 'var(--tertiary-container)' : 'transparent',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <svg ref={svgRef} width="26" height="26" viewBox="0 0 35 34">
        <path fill={active ? 'var(--on-tertiary-container)' : 'var(--error)'} d="M17.4 4 L20.6 16 H14.2 Z" />
        <path fill={active ? 'var(--on-tertiary-container)' : 'var(--primary)'} d="M17.4 30 L14.2 18 H20.6 Z" opacity=".55" />
      </svg>
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
