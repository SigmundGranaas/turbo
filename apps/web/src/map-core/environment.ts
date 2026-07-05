/** The scene-declared map environment (plan P5.2: ONE content plane — these
 *  used to be five imperative wasm setters). Serde shapes match
 *  turbomap-scene's `EnvironmentDef`: the struct is `rename_all =
 *  "kebab-case"` so its keys are hyphenated, while the lighting enum's
 *  variant *fields* keep their Rust snake_case names.
 *
 *  Lives in `map-core` (not the engine substrate) because features drive it:
 *  a feature patches the environment through [setMapEnvironment]; the live
 *  map surface subscribes via [onEnvironmentChange] and re-applies its scene
 *  (the engine diffs, so an environment-only change is a few scalars). */
export interface MapEnvironment {
  lighting:
    | { mode: 'default' }
    | { mode: 'time-tracked'; unix_seconds: number }
    | { mode: 'fixed'; azimuth_deg: number; altitude_deg: number };
  'terrain-shadows': number;
  'terrain-lit': boolean;
  'aerial-haze': boolean;
  'basemap-gain': number;
}

/** The live environment the next scene apply carries. */
let mapEnvironment: MapEnvironment = {
  lighting: { mode: 'default' },
  'terrain-shadows': 0,
  'terrain-lit': true,
  'aerial-haze': true,
  'basemap-gain': 1.0,
};
let envListener: (() => void) | undefined;

export function currentEnvironment(): MapEnvironment {
  return mapEnvironment;
}

export function setMapEnvironment(patch: Partial<MapEnvironment>): void {
  mapEnvironment = { ...mapEnvironment, ...patch };
  envListener?.();
}

/** The map surface registers here (single subscriber — the one live map). */
export function onEnvironmentChange(cb: (() => void) | undefined): void {
  envListener = cb;
}
