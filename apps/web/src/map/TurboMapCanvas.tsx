import { useEffect, useRef, useState } from 'react';
import init, { TurboMap } from 'turbomap-web';
import { buildBaseScene, type BaseLayerId } from './scene';
import { templatesFor } from './templates';
import { TileLoader } from './tileFetcher';

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
export function TurboMapCanvas({ base = 'norgeskart', threeD = false, camera, onReady, onError, onEnter3d }: Props) {
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
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    let disposed = false;
    let raf = 0;
    let map: TurboMap | null = null;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);

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

      const loader = new TileLoader(map, templatesFor(b0));
      mapRef.current = map;
      loaderRef.current = loader;
      if (import.meta.env.DEV) (window as unknown as { __map?: TurboMap }).__map = map;
      onReady?.(map);

      const frame = () => {
        if (disposed || !map) return;
        loader.pump();
        map.pump_local_tiles();
        map.render();
        raf = requestAnimationFrame(frame);
      };
      raf = requestAnimationFrame(frame);
    };

    // --- gestures: 1 pointer drags to pan, 2 pointers pinch to zoom (touch),
    // wheel to zoom (desktop). Tracking every active pointer is what makes
    // pinch work on mobile. On release we hand the gesture's velocity to the
    // engine's inertial fling (pan momentum) / zoom-fling (pinch momentum) —
    // the "physics swipe". A fresh touch cancels any running momentum
    // (touch-to-stop) by issuing a zero pan, which the engine treats as a new
    // gesture. ---
    const pointers = new Map<number, { x: number; y: number }>();
    let pinchDist = 0;
    // 3D orbit/tilt: right-mouse drag or Ctrl/⌘+drag rotates bearing (horizontal)
    // and tilts pitch (vertical) about the cursor — the "feel like a 3D app"
    // control. Starting it from 2D flips into 3D (loads terrain) via onEnter3d.
    let orbitId: number | null = null;
    let orbitPrev = { x: 0, y: 0 };
    const wantsOrbit = (e: PointerEvent) =>
      e.pointerType === 'mouse' && (e.button === 2 || e.ctrlKey || e.metaKey);
    // Velocity windows (CSS px + ms) — last few samples, used at release.
    let panV: { t: number; x: number; y: number }[] = [];
    let pinchV: { t: number; z: number }[] = []; // z = cumulative zoom levels
    let pinchZoom = 0;
    const now = () => performance.now();
    const twoPoints = () => [...pointers.values()] as [{ x: number; y: number }, { x: number; y: number }];
    // Velocity over the trailing `windowMs` of samples, in CSS px/ms.
    const velFrom = (s: { t: number; x: number; y: number }[]) => {
      if (s.length < 2) return { vx: 0, vy: 0 };
      const last = s[s.length - 1];
      let i = s.length - 1;
      while (i > 0 && last.t - s[i - 1].t < 60) i--;
      const a = s[i];
      const dt = last.t - a.t;
      if (dt <= 0) return { vx: 0, vy: 0 };
      return { vx: (last.x - a.x) / dt, vy: (last.y - a.y) / dt };
    };
    const onDown = (e: PointerEvent) => {
      if (wantsOrbit(e)) {
        orbitId = e.pointerId;
        orbitPrev = { x: e.clientX, y: e.clientY };
        canvas.setPointerCapture(e.pointerId);
        if (!threeDRef.current) onEnter3dRef.current?.(); // 2D → enter 3D so there's relief
        e.preventDefault();
        return;
      }
      if (map) map.pan_by_pixels(0, 0); // touch-to-stop any running fling
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      canvas.setPointerCapture(e.pointerId);
      panV = [{ t: now(), x: e.clientX, y: e.clientY }];
      if (pointers.size === 2) {
        const [a, b] = twoPoints();
        pinchDist = Math.hypot(a.x - b.x, a.y - b.y);
        pinchZoom = 0;
        pinchV = [{ t: now(), z: 0 }];
      }
    };
    const onUp = (e: PointerEvent) => {
      if (orbitId === e.pointerId) {
        orbitId = null;
        try {
          canvas.releasePointerCapture(e.pointerId);
        } catch {
          /* already released */
        }
        return;
      }
      const wasPinch = pointers.size >= 2;
      const center = wasPinch ? twoPoints() : null;
      pointers.delete(e.pointerId);
      try {
        canvas.releasePointerCapture(e.pointerId);
      } catch {
        /* pointer already released */
      }
      if (!map) return;
      if (wasPinch) {
        // Pinch ended: hand zoom-rate (levels/sec) to the zoom fling.
        const last = pinchV[pinchV.length - 1];
        let i = pinchV.length - 1;
        while (i > 0 && last && last.t - pinchV[i - 1].t < 60) i--;
        const a = pinchV[i];
        const dt = last && a ? last.t - a.t : 0;
        const zv = dt > 0 ? ((last.z - a.z) / dt) * 1000 : 0;
        if (center && Math.abs(zv) > 0.3) {
          const [p, q] = center;
          map.zoom_fling(zv, ((p.x + q.x) / 2) * dpr, ((p.y + q.y) / 2) * dpr);
        }
        pinchDist = 0;
      } else if (pointers.size === 0) {
        // Last finger lifted off a drag: fling by release velocity (px/s).
        const { vx, vy } = velFrom(panV);
        const speed = Math.hypot(vx, vy) * 1000; // px/s
        if (speed > 80) map.fling(vx * 1000 * dpr, vy * 1000 * dpr);
      }
    };
    const onMove = (e: PointerEvent) => {
      if (orbitId === e.pointerId && map) {
        const dx = e.clientX - orbitPrev.x;
        const dy = e.clientY - orbitPrev.y;
        orbitPrev = { x: e.clientX, y: e.clientY };
        // drag right → rotate view; drag up → tilt toward the horizon. Pivot
        // about the cursor so the point under it stays put.
        map.orbit_around(-dx * 0.45, -dy * 0.4, e.clientX * dpr, e.clientY * dpr);
        return;
      }
      if (!map || !pointers.has(e.pointerId)) return;
      const prev = pointers.get(e.pointerId)!;
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pointers.size >= 2) {
        const [a, b] = twoPoints();
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        if (pinchDist > 0 && d > 0) {
          const factor = d / pinchDist;
          map.zoom_around(factor, ((a.x + b.x) / 2) * dpr, ((a.y + b.y) / 2) * dpr);
          pinchZoom += Math.log2(factor);
          pinchV.push({ t: now(), z: pinchZoom });
          if (pinchV.length > 8) pinchV.shift();
        }
        pinchDist = d;
      } else {
        map.pan_by_pixels((e.clientX - prev.x) * dpr, (e.clientY - prev.y) * dpr);
        panV.push({ t: now(), x: e.clientX, y: e.clientY });
        if (panV.length > 8) panV.shift();
      }
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (!map) return;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      // Eased zoom (retargets each tick) so the wheel glides instead of
      // snapping per notch; focus stays under the cursor.
      map.zoom_around_animated(factor, e.clientX * dpr, e.clientY * dpr, 180);
    };
    const onResize = () => {
      if (!map) return;
      const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      canvas.width = w;
      canvas.height = h;
      map.resize(w, h);
    };

    // Right-drag orbits the 3D camera, so the browser context menu must not pop.
    const onContextMenu = (e: Event) => e.preventDefault();
    canvas.addEventListener('pointerdown', onDown);
    canvas.addEventListener('pointerup', onUp);
    canvas.addEventListener('pointercancel', onUp);
    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('wheel', onWheel, { passive: false });
    canvas.addEventListener('contextmenu', onContextMenu);
    window.addEventListener('resize', onResize);

    void boot();

    return () => {
      disposed = true;
      cancelAnimationFrame(raf);
      canvas.removeEventListener('pointerdown', onDown);
      canvas.removeEventListener('pointerup', onUp);
      canvas.removeEventListener('pointercancel', onUp);
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('wheel', onWheel);
      canvas.removeEventListener('contextmenu', onContextMenu);
      window.removeEventListener('resize', onResize);
      mapRef.current = null;
      loaderRef.current = null;
      map?.free?.();
    };
    // Init the GPU map once per mount (+ on initial-camera identity change).
    // Base-layer switches are handled in place by the effect below — no
    // re-init, so the camera position is preserved.
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
      <canvas ref={canvasRef} className="map-canvas" />
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
