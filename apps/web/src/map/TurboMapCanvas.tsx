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
  camera?: CameraInit;
  /** Called once the GPU map is live (for wiring overlays/controls later). */
  onReady?: (map: TurboMap) => void;
  /** Called with a human-readable reason if WebGPU/init fails. */
  onError?: (message: string) => void;
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
export function TurboMapCanvas({ base = 'norgeskart', camera, onReady, onError }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mapRef = useRef<TurboMap | null>(null);
  const loaderRef = useRef<TileLoader | null>(null);
  const baseRef = useRef<BaseLayerId>(base);
  baseRef.current = base;
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
      map.apply_scene(JSON.stringify(buildBaseScene(b0)));

      const loader = new TileLoader(map, templatesFor(b0));
      mapRef.current = map;
      loaderRef.current = loader;
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

    // --- gestures: drag to pan, wheel to zoom ---
    let dragging = false;
    let lastX = 0;
    let lastY = 0;
    const onDown = (e: PointerEvent) => {
      dragging = true;
      lastX = e.clientX;
      lastY = e.clientY;
      canvas.setPointerCapture(e.pointerId);
    };
    const onUp = () => {
      dragging = false;
    };
    const onMove = (e: PointerEvent) => {
      if (!dragging || !map) return;
      map.pan_by_pixels((e.clientX - lastX) * dpr, (e.clientY - lastY) * dpr);
      lastX = e.clientX;
      lastY = e.clientY;
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (!map) return;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      map.zoom_around(factor, e.clientX * dpr, e.clientY * dpr);
    };
    const onResize = () => {
      if (!map) return;
      const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      canvas.width = w;
      canvas.height = h;
      map.resize(w, h);
    };

    canvas.addEventListener('pointerdown', onDown);
    canvas.addEventListener('pointerup', onUp);
    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('wheel', onWheel, { passive: false });
    window.addEventListener('resize', onResize);

    void boot();

    return () => {
      disposed = true;
      cancelAnimationFrame(raf);
      canvas.removeEventListener('pointerdown', onDown);
      canvas.removeEventListener('pointerup', onUp);
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('wheel', onWheel);
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

  // Switch base layer in place: repoint the tile fetcher + re-apply the scene
  // (same `basemap` source id → the engine re-ingests from the new URL),
  // keeping the current camera. No-op until the map has booted.
  useEffect(() => {
    const map = mapRef.current;
    const loader = loaderRef.current;
    if (!map || !loader) return;
    loader.setTemplates(templatesFor(base));
    map.apply_scene(JSON.stringify(buildBaseScene(base)));
  }, [base]);

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
