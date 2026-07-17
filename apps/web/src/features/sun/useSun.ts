import { useEffect } from 'react';
import { deriveMapEnvironment, setMapEnvironment, type DerivedEnv } from '../../map-core';
import { useUiStore } from '../../store/uiStore';
import { useSunStore } from './sunStore';

/** Cast-shadow strength while the sun is on — strong enough to read on relief
 *  (matches Android's `SUN_MODE_SHADOW_STRENGTH`). */
const SUN_MODE_SHADOW_STRENGTH = 0.85;

/** UTC seconds for today's date at a fractional local `hour`. The engine solves
 *  the sun's azimuth/altitude from this instant at the camera location. */
function unixForHourToday(hour: number): number {
  const d = new Date();
  d.setHours(Math.floor(hour), Math.round((hour - Math.floor(hour)) * 60), 0, 0);
  return d.getTime() / 1000;
}

/**
 * The map's "control center": the two layers-sheet sliders (3D terrain + Sun)
 * reduced to a derived scene environment, with the lighting side-effects applied
 * to the live scene.
 *
 * The **hard decoupling rule** lives here by omission: this hook never touches
 * the camera. Turning the sun on lights the relief top-down; turning 3D on only
 * unlocks tilt *gestures*. Neither slider sets a pitch — that's the caller's job,
 * and only in response to a gesture. Mirrors Android's `mapEnvironment` +
 * `DriveSunMode` split.
 */
export function useSun(): {
  threeDLevel: number;
  sunLevel: number;
  env: DerivedEnv;
  setThreeDLevel: (v: number) => void;
  setSunLevel: (v: number) => void;
} {
  const threeDLevel = useUiStore((s) => s.threeDLevel);
  const sunLevel = useSunStore((s) => s.level);
  const env = deriveMapEnvironment(threeDLevel, sunLevel);

  // Drive the scene lighting from the derived sun position. Sun on ⇒ time-tracked
  // lighting raked to `sunHour` + cast shadows + terrain-lit relief. Sun off ⇒
  // default lighting, no shadows, and the bare bright basemap draws over any 3D
  // relief (the heavy per-fragment shading path is skipped — a big perf win).
  // Runs purely off the reduced env — no camera involved.
  useEffect(() => {
    if (env.sunHour != null) {
      setMapEnvironment({
        lighting: { mode: 'time-tracked', unix_seconds: unixForHourToday(env.sunHour) },
        'terrain-shadows': SUN_MODE_SHADOW_STRENGTH,
        'terrain-lit': true,
      });
    } else {
      setMapEnvironment({
        lighting: { mode: 'default' },
        'terrain-shadows': 0,
        'terrain-lit': false,
      });
    }
  }, [env.sunHour]);

  return {
    threeDLevel,
    sunLevel,
    env,
    setThreeDLevel: (v) => useUiStore.getState().setThreeDLevel(v),
    setSunLevel: (v) => useSunStore.getState().setLevel(v),
  };
}
