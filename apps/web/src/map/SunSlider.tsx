import { Glass } from '../ui/Glass';

/** Time-of-day slider shown while Sun mode is on. Drives the engine's solar
 *  position (`set_sun_time`) so the user can sweep the lighting from dawn to
 *  dusk instead of being stuck at the real-clock time (which can be dark). */
export function SunSlider({
  dark,
  hour,
  onChange,
}: {
  dark: boolean;
  /** Hours past local midnight, 0–24 (fractional). */
  hour: number;
  onChange: (hour: number) => void;
}) {
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
        max={24}
        step={0.25}
        value={hour}
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
