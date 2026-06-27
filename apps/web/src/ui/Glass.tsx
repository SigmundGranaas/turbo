import type { CSSProperties, ReactNode } from 'react';
import { Icon } from './Icon';

/** The web identity: floating controls are frosted, translucent warm surfaces
 *  over the live 3D map. No GPU backdrop-blur (kept fast across many frames) —
 *  legibility comes from opacity + a soft border + drop shadow. Ported from the
 *  design's `webKit.jsx` glass tokens. */
type GlassLevel = 'panel' | 'high' | 'chip';

export function glassBg(dark: boolean, level: GlassLevel = 'high'): string {
  const a = { panel: dark ? 0.9 : 0.91, high: dark ? 0.86 : 0.87, chip: dark ? 0.78 : 0.8 }[level];
  const base = dark ? '26,17,15' : '255,248,246';
  return `rgba(${base},${a})`;
}
export function glassBorder(dark: boolean): string {
  return dark ? '1px solid rgba(255,210,196,.12)' : '1px solid rgba(255,255,255,.6)';
}
export function glassShadow(dark: boolean): string {
  return dark
    ? '0 2px 8px rgba(0,0,0,.30), 0 12px 34px rgba(0,0,0,.42)'
    : '0 2px 8px rgba(50,28,18,.10), 0 10px 30px rgba(50,28,18,.14)';
}

export function Glass({
  dark,
  level = 'high',
  radius = 28,
  style = {},
  children,
  onClick,
}: {
  dark: boolean;
  level?: GlassLevel;
  radius?: number;
  style?: CSSProperties;
  children?: ReactNode;
  onClick?: () => void;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        background: glassBg(dark, level),
        border: glassBorder(dark),
        borderRadius: radius,
        boxShadow: glassShadow(dark),
        fontFamily: 'var(--font-sans)',
        boxSizing: 'border-box',
        ...style,
      }}
    >
      {children}
    </div>
  );
}

export function Divider({ inset, vertical }: { inset?: boolean; vertical?: boolean }) {
  return vertical ? (
    <div style={{ width: 1, alignSelf: 'stretch', background: 'var(--outline-variant)', margin: inset ? '6px 0' : 0 }} />
  ) : (
    <div style={{ height: 1, background: 'var(--outline-variant)', margin: inset ? '0 10px' : 0 }} />
  );
}

export function GlassIconBtn({
  icon,
  active,
  fill,
  size = 48,
  iconSize = 24,
  badge,
  title,
  onClick,
}: {
  icon: string;
  active?: boolean;
  fill?: boolean;
  size?: number;
  iconSize?: number;
  badge?: number | null;
  title?: string;
  onClick?: () => void;
}) {
  return (
    <button
      className="tm-icon-btn"
      title={title}
      aria-label={title}
      onClick={onClick}
      disabled={!onClick}
      style={{
        width: size,
        height: size,
        borderRadius: 18,
        position: 'relative',
        cursor: 'pointer',
        border: 'none',
        background: active ? 'var(--tertiary-container)' : 'transparent',
        color: active ? 'var(--on-tertiary-container)' : 'var(--primary)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        transition: 'background .2s',
      }}
    >
      <Icon name={icon} size={iconSize} fill={Boolean(fill || active)} weight={500} />
      {badge != null && (
        <span
          style={{
            position: 'absolute',
            top: 5,
            right: 5,
            minWidth: 16,
            height: 16,
            padding: '0 4px',
            borderRadius: 8,
            background: 'var(--error)',
            color: '#fff',
            font: '700 10px/16px var(--font-sans)',
            textAlign: 'center',
          }}
        >
          {badge}
        </span>
      )}
    </button>
  );
}

type BtnTone = 'primary' | 'tonal' | 'tertiary' | 'surface' | 'text' | 'error';

export function Btn({
  label,
  icon,
  tone = 'primary',
  size = 'reg',
  full,
  trailingIcon,
  onClick,
}: {
  label: string;
  icon?: string;
  tone?: BtnTone;
  size?: 'sm' | 'reg' | 'lg';
  full?: boolean;
  trailingIcon?: string;
  onClick?: () => void;
}) {
  const tones: Record<BtnTone, [string, string]> = {
    primary: ['var(--primary)', 'var(--on-primary)'],
    tonal: ['var(--secondary-container)', 'var(--on-secondary-container)'],
    tertiary: ['var(--tertiary-container)', 'var(--on-tertiary-container)'],
    surface: ['var(--surface-container-high)', 'var(--primary)'],
    text: ['transparent', 'var(--primary)'],
    error: ['var(--error)', '#fff'],
  };
  const [bg, fg] = tones[tone];
  const h = size === 'lg' ? 56 : size === 'sm' ? 38 : 48;
  return (
    <button
      className="tm-btn"
      onClick={onClick}
      disabled={!onClick}
      style={{
        height: h,
        padding: size === 'sm' ? '0 16px' : '0 24px',
        border: 'none',
        cursor: 'pointer',
        background: bg,
        color: fg,
        borderRadius: 9999,
        width: full ? '100%' : undefined,
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 9,
        font: `600 ${size === 'lg' ? 16 : 15}px/1 var(--font-sans)`,
        letterSpacing: 0.1,
        boxShadow: tone === 'primary' || tone === 'error' ? 'var(--elevation-1)' : 'none',
      }}
    >
      {icon && <Icon name={icon} size={size === 'sm' ? 18 : 20} color={fg} fill weight={500} />}
      {label}
      {trailingIcon && <Icon name={trailingIcon} size={18} color={fg} weight={500} />}
    </button>
  );
}
