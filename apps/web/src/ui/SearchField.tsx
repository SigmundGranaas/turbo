import { Glass } from './Glass';
import { Icon } from './Icon';
import { Cookie } from './Cookie';

/** The floating search pill over the map (the app's primary entry point). A real
 *  input so it's usable now; the fused results dropdown lands with the search
 *  feature (port doc 12). */
export function SearchField({
  dark,
  value,
  onChange,
  onFocus,
  placeholder = 'Search places, coordinates…',
  width = 380,
  leading = 'search',
  avatar,
  onAvatar,
}: {
  dark: boolean;
  value?: string;
  onChange?: (v: string) => void;
  onFocus?: () => void;
  placeholder?: string;
  width?: number | string;
  leading?: string;
  avatar?: string;
  onAvatar?: () => void;
}) {
  return (
    <Glass
      dark={dark}
      radius={9999}
      level="high"
      style={{ height: 56, width, padding: '0 8px 0 16px', display: 'flex', alignItems: 'center', gap: 10 }}
    >
      <Icon name={leading} size={22} color="var(--on-surface-variant)" />
      <input className="tm-field"
        value={value ?? ''}
        onChange={(e) => onChange?.(e.target.value)}
        onFocus={onFocus}
        placeholder={placeholder}
        style={{
          flex: 1,
          minWidth: 0,
          border: 'none',
          outline: 'none',
          background: 'transparent',
          font: '400 16px/24px var(--font-sans)',
          color: 'var(--on-surface)',
        }}
      />
      {avatar && (
        <button
          onClick={onAvatar}
          title="Account"
          style={{ border: 'none', background: 'transparent', padding: 0, cursor: 'pointer' }}
        >
          <Cookie size={36} lobes={7} fill="var(--primary)">
            <span style={{ font: '600 14px/1 var(--font-sans)', color: 'var(--on-primary)' }}>{avatar}</span>
          </Cookie>
        </button>
      )}
    </Glass>
  );
}
