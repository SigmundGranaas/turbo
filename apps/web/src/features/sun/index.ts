/** `sun` feature slice — time-of-day sun-lighting + cast shadows on the 3D
 *  terrain. Public API: the `useSun()` tool hook (on/hour/toggle/setHour) and
 *  the `<SunSlider>` control. Owns its on/off + time-of-day state. */
export { useSun } from './useSun';
export { SunSlider } from './SunSlider';
