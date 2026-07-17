import { Glass } from '../../ui/Glass';

/** Dawn/dusk arc the normalized sun level sweeps — mirrors the reducer's
 *  `SUN_HOUR_DAWN..SUN_HOUR_DUSK` so the label the user reads matches the sun
 *  position the scene is actually lit by. */
const SUN_HOUR_DAWN = 4;
const SUN_HOUR_DUSK = 22;

/** Hours-past-midnight for a normalized sun level `[0,1]`. */
export function sunLevelToHour(level: number): number {
  return SUN_HOUR_DAWN + level * (SUN_HOUR_DUSK - SUN_HOUR_DAWN);
}

/** Bottom-centred time-of-day scrubber shown while the Sun slider is on. Drives
 *  the normalized sun level (the layers-sheet slider drives the same state), so
 *  the user can rake the light from dawn to dusk. Moving it moves the sun's
 *  position/time — never the camera. */
export function SunSlider({
  dark,
  level,
  onChange,
}: {
  dark: boolean;
  /** Normalized sun level, 0–1. */
  level: number;
  onChange: (level: number) => void;
}) {
  const hour = sunLevelToHour(level);
  const hh = Math.floor(hour) % 24;
  const mm = Math.round((hour - Math.floor(hour)) * 60);
  const label = `${String(hh).padStart(2, '0')}:${String(mm % 60).padStart(2, '0')}`;
  // Sun icon shifts with the time so the control reads as "where the sun is".
  const icon = hour < 6 || hour >= 21 ? 'bedtime' : hour < 9 || hour >= 18 ? 'wb_twilight' : 'light_mode';
  return (
    <Glass
      dark={dark}
      radius={22}
      style={{ padding: '8px 14px', display: 'flex', alignItems: 'center', gap: 12, width: 'min(440px, 78vw)' }}
    >
      <span className="material-symbols-outlined" style={{ fontSize: 20, color: 'var(--primary)' }}>
        {icon}
      </span>
      <input
        type="range"
        min={0}
        max={1}
        step={0.01}
        value={level}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        aria-label="Time of day"
        style={{ flex: 1, accentColor: 'var(--primary)', cursor: 'pointer' }}
      />
      <span
        style={{
          font: '600 13px/1 var(--font-sans)',
          color: 'var(--on-surface)',
          minWidth: 40,
          textAlign: 'right',
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        {label}
      </span>
    </Glass>
  );
}
