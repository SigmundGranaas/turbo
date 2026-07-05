import { useMapEngine } from '../../map-core';
import { setMapEnvironment } from '../../map-core';
import { useUiStore } from '../../store/uiStore';
import { useSunStore, nowHour } from './sunStore';

interface Cam {
  lat: number;
  lng: number;
  zoom: number;
  pitch: number;
  bearing: number;
}

/** The sun tool's behaviour: toggle sun-mode and sweep the time-of-day, applied
 *  to the live engine. Turning sun on ensures 3D (sun-lighting needs relief) and
 *  enables cast shadows; turning it off clears both. Reads the engine from the
 *  kernel context — no prop-drilled map handle. */
export function useSun() {
  const engine = useMapEngine();
  const on = useSunStore((s) => s.on);
  const hour = useSunStore((s) => s.hour);

  // Apply a time-of-day (hours past local midnight) as the engine sun time.
  const applyHour = (h: number) => {
    if (!engine) return;
    const d = new Date();
    d.setHours(Math.floor(h), Math.round((h - Math.floor(h)) * 60), 0, 0);
    setMapEnvironment({ lighting: { mode: 'time-tracked', unix_seconds: d.getTime() / 1000 } });
  };

  const toggle = () => {
    if (!engine) return;
    const next = !useSunStore.getState().on;
    if (next) {
      // Sun-lighting needs relief to read — ensure 3D first (tilt + terrain).
      if (!useUiStore.getState().threeD) {
        try {
          const c = JSON.parse(engine.camera_json()) as Cam;
          engine.set_camera(c.lat, c.lng, c.zoom, 60, c.bearing);
        } catch {
          /* camera not ready */
        }
        useUiStore.getState().setThreeD(true);
      }
      const h = nowHour(); // start at the real clock, then the slider sweeps it
      useSunStore.getState().setHour(h);
      applyHour(h);
      // Cast shadows (peaks shadow the valleys) — what makes relief read as
      // distinct, like the native app. Only on in sun mode.
      setMapEnvironment({ 'terrain-shadows': 0.7 });
    } else {
      setMapEnvironment({ lighting: { mode: 'default' } });
      setMapEnvironment({ 'terrain-shadows': 0 });
    }
    useSunStore.getState().setOn(next);
  };

  const setHour = (h: number) => {
    useSunStore.getState().setHour(h);
    applyHour(h);
  };

  return { on, hour, toggle, setHour };
}
