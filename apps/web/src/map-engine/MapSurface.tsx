import { useEffect, useRef, useState } from 'react';
import init, { TurboMap } from 'turbomap-web';
import { useMapEnginePublisher } from '../map-core';
import { buildBaseScene, onEnvironmentChange, type BaseLayerId } from './scene';
import { templatesFor } from './templates';
import { TileLoader } from './tileFetcher';
import { attachMapGestures } from './gestures';

export interface CameraInit {
  lat: number;
  lng: number;
  zoom: number;
}

interface Props {
  base?: BaseLayerId;
  /** 3D mode — adds the DEM terrain to the scene (relief + sun-lit ground) and
   *  enables orbit/tilt gestures. In 2D the map stays flat (no terrain shading,
   *  exact overlay registration). */
  threeD?: boolean;
  camera?: CameraInit;
  /** Called once the GPU map is live (for wiring overlays/controls later). */
  onReady?: (map: TurboMap) => void;
  /** Called with a human-readable reason if WebGPU/init fails. */
  onError?: (message: string) => void;
  /** Fired when an orbit/tilt gesture starts while in 2D — the host flips to
   *  3D (loads terrain) so the tilt has relief to orbit around. */
  onEnter3d?: () => void;
  /** A click (mouse) / tap (touch) on the map that wasn't a drag/double/
   *  long-press. `pointerType` lets the host branch (mouse=add, touch=select).
   *  Coordinates are CSS px. */
  onTap?: (x: number, y: number, pointerType: string) => void;
  /** A touch long-press — the mobile "add marker" gesture. CSS px. */
  onLongPress?: (x: number, y: number) => void;
  /** The terrain point a live 3D orbit/tilt gesture pivots around (or `null`
   *  when it ends) — the host pins it on the relief. */
  onOrbit?: (anchor: { lat: number; lng: number } | null) => void;
}

const DEFAULT_CAMERA: CameraInit = { lat: 60.39, lng: 5.32, zoom: 12 }; // Bergen

// Module-scoped one-time WASM init promise: `init()` fetches + instantiates the
// .wasm once per page; multiple canvases (or React StrictMode's double-mount)
// must not re-run it.
let wasmReady: Promise<unknown> | null = null;
function ensureWasm(): Promise<unknown> {
  if (!wasmReady) wasmReady = init();
  return wasmReady;
}

/** The map surface: owns the WASM `TurboMap`, the requestAnimationFrame render
 *  loop, host-driven tile loading, and pointer/wheel gestures. The React
 *  analogue of Android's `TurbomapMapView` — a thin host around the shared
 *  engine. Everything map-visual lives in the engine; this component only feeds
 *  it input + tiles and pumps frames. */
export function MapSurface({ base = 'norgeskart', threeD = false, camera, onReady, onError, onEnter3d, onTap, onLongPress, onOrbit }: Props) {
  const publish = useMapEnginePublisher();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mapRef = useRef<TurboMap | null>(null);
  const loaderRef = useRef<TileLoader | null>(null);
  const baseRef = useRef<BaseLayerId>(base);
  baseRef.current = base;
  // Latest 3D state, read inside the init closure + gesture handlers (which are
  // set up once and must see the current value, not the mount-time capture).
  const threeDRef = useRef<boolean>(threeD);
  threeDRef.current = threeD;
  const onEnter3dRef = useRef<(() => void) | undefined>(onEnter3d);
  onEnter3dRef.current = onEnter3d;
  const onTapRef = useRef<Props['onTap']>(onTap);
  onTapRef.current = onTap;
  const onLongPressRef = useRef<Props['onLongPress']>(onLongPress);
  onLongPressRef.current = onLongPress;
  const onOrbitRef = useRef<Props['onOrbit']>(onOrbit);
  onOrbitRef.current = onOrbit;
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    let disposed = false;
    let raf = 0;
    let map: TurboMap | null = null;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    // Render-on-demand: the WASM render() is a full GPU pass and in sun mode it's
    // expensive (per-fragment terrain shadow march + AO + sky). `dirty` is set by
    // any input/state change; between changes we draw at a low safety cadence
    // instead of pegging the GPU at 60 fps while the view sits still.
    let dirty = true;
    let lastRender = 0;
    const invalidate = () => { dirty = true; };

    const fail = (msg: string) => {
      if (disposed) return;
      setError(msg);
      onError?.(msg);
    };

    const boot = async () => {
      if (!('gpu' in navigator)) {
        fail(
          'WebGPU is not available in this browser. Use Chrome/Edge, Safari 18+, or Firefox with WebGPU enabled.',
        );
        return;
      }
      try {
        await ensureWasm();
      } catch (e) {
        fail(`Failed to load the map engine: ${String(e)}`);
        return;
      }
      if (disposed) return;

      const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      canvas.width = w;
      canvas.height = h;

      const c = camera ?? DEFAULT_CAMERA;
      try {
        map = await TurboMap.create(canvas, w, h, c.lat, c.lng, c.zoom);
      } catch (e) {
        fail(`GPU init failed: ${String(e)}`);
        return;
      }
      if (disposed || !map) {
        map?.free?.();
        return;
      }

      const b0 = baseRef.current;
      map.apply_scene(JSON.stringify(buildBaseScene(b0, threeDRef.current)));
      // Environment changes (sun mode, haze, shadows) re-apply the scene —
      // ONE content plane; the engine diffs so this is a few scalars.
      onEnvironmentChange(() => {
        mapRef.current?.apply_scene(
          JSON.stringify(buildBaseScene(baseRef.current, threeDRef.current)),
        );
      });

      const loader = new TileLoader(map, templatesFor(b0));
      mapRef.current = map;
      loaderRef.current = loader;
      if (import.meta.env.DEV) (window as unknown as { __map?: TurboMap }).__map = map;
      // Publish the live engine to the kernel context (the seam features read via
      // `useMapEngine()`); `onReady` is kept for the host's existing `mapRef`
      // wiring until overlays migrate off it.
      publish(map);
      onReady?.(map);

      const frame = (ts: number) => {
        if (disposed || !map) return;
        loader.pump();
        map.pump_local_tiles();
        // Draw on change / while animating (camera moves, tile fade-in), else a
        // ~5 fps safety tick so any missed state change still lands quickly. This
        // keeps idle sun-mode from continuously re-running the shadow march.
        if (dirty || map.is_animating()) {
          map.render();
          dirty = false;
          lastRender = ts;
        } else if (ts - lastRender > 200) {
          map.render();
          lastRender = ts;
        }
        raf = requestAnimationFrame(frame);
      };
      raf = requestAnimationFrame(frame);
    };

    // All pointer/wheel/keyboard gestures live in the controller; it emits
    // semantic taps/long-presses back to the host. `getMap` reads the async
    // `map` binding live (null until boot finishes).
    const detachGestures = attachMapGestures(canvas, dpr, {
      getMap: () => map,
      is3d: () => threeDRef.current,
      onEnter3d: () => onEnter3dRef.current?.(),
      onTap: (x, y, t) => onTapRef.current?.(x, y, t),
      onLongPress: (x, y) => onLongPressRef.current?.(x, y),
      onOrbit: (a) => onOrbitRef.current?.(a),
    });

    // Any direct interaction marks the next frame dirty so it draws immediately
    // (the gesture controller mutates the camera from these same events). Capture
    // + passive so it fires regardless of how the controller handles the event.
    const INPUT = ['pointerdown', 'pointermove', 'wheel', 'touchstart', 'touchmove'] as const;
    INPUT.forEach((e) => canvas.addEventListener(e, invalidate, { passive: true, capture: true }));

    const onResize = () => {
      if (!map) return;
      const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      canvas.width = w;
      canvas.height = h;
      map.resize(w, h);
      invalidate();
    };
    window.addEventListener('resize', onResize);

    void boot();

    return () => {
      disposed = true;
      onEnvironmentChange(undefined);
      cancelAnimationFrame(raf);
      detachGestures();
      INPUT.forEach((e) => canvas.removeEventListener(e, invalidate, { capture: true } as EventListenerOptions));
      window.removeEventListener('resize', onResize);
      publish(null);
      mapRef.current = null;
      loaderRef.current = null;
      map?.free?.();
    };
    // Init the GPU map once per mount (+ on initial-camera identity change).
    // Base-layer switches are handled in place by the effect below — no
    // re-init, so the camera position is preserved.
  }, [camera, onReady, onError]);

  // Re-apply the scene when the base layer OR 3D state changes: repoint the
  // tile fetcher + rebuild the scene (terrain is added only in 3D). Camera is
  // preserved. No-op until the map has booted.
  useEffect(() => {
    const map = mapRef.current;
    const loader = loaderRef.current;
    if (!map || !loader) return;
    loader.setTemplates(templatesFor(base));
    map.apply_scene(JSON.stringify(buildBaseScene(base, threeD)));
  }, [base, threeD]);

  const isWebGpu = error?.toLowerCase().includes('webgpu');
  return (
    <div className="map-root">
      <canvas
        ref={canvasRef}
        className="map-canvas"
        tabIndex={0}
        role="application"
        aria-label="Map — drag to pan, scroll or +/− to zoom, arrow keys to move, Shift+arrows to rotate/tilt"
      />
      {error && (
        <div className="map-error">
          <div className="map-error-card">
            <span className="material-symbols-outlined" style={{ fontSize: 40, color: 'var(--primary)' }}>
              {isWebGpu ? 'desktop_access_disabled' : 'cloud_off'}
            </span>
            <div className="map-error-title">{isWebGpu ? 'This browser can’t run the 3D map' : 'Couldn’t start the map'}</div>
            <div className="map-error-body">{error}</div>
          </div>
        </div>
      )}
    </div>
  );
}
