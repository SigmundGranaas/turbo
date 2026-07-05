import { useEffect, useRef } from 'react';
import { useMapEngine } from './MapEngineContext';
import type { MapEngine } from './engine';

/** Device-pixel ratio the engine renders at (capped at 2 to bound GPU work). */
export const viewportDpr = (): number => Math.min(window.devicePixelRatio || 1, 2);

/** Drive a per-frame projection against the live engine. `draw` runs every
 *  animation frame with the booted engine + current DPR; overlays use it to
 *  position DOM/SVG via `engine.project()` by mutating refs directly — never
 *  React state — so the camera can move at 60fps without re-rendering the tree.
 *  No-op until the engine has booted; cleans up its rAF on unmount.
 *
 *  Map CONTENT (pins, route lines, the location dot) is scene-declared and
 *  drawn by the engine (plan P6.3) — this hook is only for interactive chrome
 *  anchored to a geo point: waypoint drag handles, the click ring, popup
 *  anchors. */
export function useProjectedLayer(draw: (engine: MapEngine, dpr: number) => void): void {
  const engine = useMapEngine();
  const drawRef = useRef(draw);
  drawRef.current = draw;
  useEffect(() => {
    if (!engine) return;
    let raf = 0;
    const tick = () => {
      drawRef.current(engine, viewportDpr());
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [engine]);
}
