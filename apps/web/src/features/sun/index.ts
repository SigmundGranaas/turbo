/** `sun` feature slice — the map's terrain + light "control center". The two
 *  layers-sheet sliders (3D terrain + Sun) reduce to a scene environment via
 *  `deriveMapEnvironment`; this slice applies the sun-lighting side-effects and
 *  exposes the derived env + setters. Public API: the `useSun()` hook and the
 *  bottom `<SunSlider>` time scrubber. Neither slider ever moves the camera. */
export { useSun } from './useSun';
export { SunSlider, sunLevelToHour } from './SunSlider';
